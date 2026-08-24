# Tetris 1 대 1 강화학습 조사 정리

조사 기준 시각: `2026-08-24T15:05:44+09:00`

## 1. 문제 정의

학습 환경은 고정된 TETR.IO mechanics profile을 사용하는 2인 zero-sum stochastic game으로 본다.

- 관측: 양쪽 visible board, active/hold/next, incoming garbage packet과 timing, combo, B2B/Surge, piece count, round terminal 정보
- 비공개 정보: 상대의 내부 정책 상태와 아직 관측되지 않은 RNG future
- 행동: 실제 timing/handling에서 도달 가능한 legal placement afterstate
- 목적: round 승리 `+1`, 패배 `-1`, 무승부 `0`
- 제약: 동일한 wall-time, sample, memory와 inference-latency 예산에서 비교

배치 abstraction은 decision 횟수를 크게 줄이지만 mechanics를 생략해서는 안 된다. move generator는 회전·kick·DAS/ARR/SDF·gravity·lock 조건을 만족하는 input path를 함께 산출해야 한다. 이후 필요하면 placement policy와 frame-input executor를 계층적으로 분리한다.

## 2. 왜 처음부터 CNN을 고정하지 않는가

### 고전 연구가 주는 근거

Tetris 연구에서는 board를 그대로 convolution하는 방식보다, landing height, eroded cells, row/column transitions, holes, wells 같은 구조적 feature와 afterstate 평가가 오랫동안 강한 기준선이었다. Scherrer 등은 approximate modified policy iteration과 compact representation으로 적은 sample에서 경쟁력 있는 성능을 보였고, Algorta와 Şimşek의 survey는 작은 구현 차이도 점수에 큰 영향을 준다고 정리한다.

이 결과를 1 대 1에 그대로 복사할 수는 없지만 다음 두 결론은 유효하다.

1. 작은 모델이 rule-correct simulator에서 훨씬 많은 경험을 처리하면 큰 CNN보다 나을 수 있다.
2. board feature만으로는 garbage timing, attack pressure, B2B/Surge와 상대 상태를 표현할 수 없으므로 versus context를 추가해야 한다.

### 최근 연구가 주는 근거와 한계

2026년 bitboard/afterstate preprint는 자체 solo Tetris 조건에서 bitboard simulator의 큰 속도 향상과 작은 afterstate actor의 효율을 보고했다. 이는 native bitboard와 afterstate scorer의 유력한 근거지만, 10×10 solo 설정과 다른 generator에서 얻은 결과이므로 TL 1 대 1이나 PPO의 우월성을 증명하지 않는다.

따라서 모델은 다음 단계별로 승격한다.

| 단계 | 후보 | 역할 | 승격 조건 |
|---|---|---|---|
| M0 | normalized linear afterstate scorer | 설명 가능한 최저 비용 기준선 | 필수 |
| M1 | 작은 shared MLP scorer + value head | board/versus feature의 비선형 상호작용 | M0보다 동일 예산 cross-play가 유의하게 강함 |
| M2 | tiny spatial encoder + scalar context | feature가 놓치는 국소 구조 학습 | M1보다 wall-time·latency 포함 Pareto 우위 |
| M3 | M1/M2가 유도하는 shallow search | tactical spike/downstack lookahead | node budget 내 held-out 성능 향상 |

“유의하게 강함”은 단일 self-play reward가 아니라 paired seed 대국의 승률 신뢰구간과 상대 population 전체의 payoff로 판단한다.

## 3. 입력 표현

### 3.1 공통 board/afterstate feature

- aggregate/max height와 column height profile
- holes와 hole depth, covered cells
- row/column transitions
- well depth와 well sum
- bumpiness와 surface parity
- cleared lines, eroded cells, landing height
- reachable cavity와 garbage-hole 접근 비용

### 3.2 versus feature

- 이번 placement의 base/final attack과 cancel 결과
- incoming garbage의 packet별 line 수·도착/activation까지 남은 frame
- 현재 combo, B2B count, stored Surge와 break 시 release schedule
- opener phase의 남은 piece와 doubled-cancel 조건
- 양쪽 board danger 차이, top-out 여유 차이
- 다음 N pieces와 hold에서 가능한 attack/downstack 기회 요약
- 현재 player와 opponent의 piece count·tempo 차이

feature는 profile별 상한이나 robust scale로 정규화하고, clipped range와 단위를 문서화한다. 정보 누출을 막기 위해 관측되지 않은 future bag, opponent policy state, fixture answer를 입력에 넣지 않는다.

## 4. 학습 알고리즘 선택

### 4.1 필수 기준선

- hand-tuned heuristic: simulator와 feature 방향성 검증
- noisy cross-entropy 또는 evolution strategy 기반 linear optimizer: gradient-free 저비용 기준선
- shallow search + linear evaluator: 학습 없이 얻을 수 있는 tactical 기준선
- frozen scripted style들: 공격형, downstack형, B2B 유지형, 안전형

### 4.2 RL 후보

- 작은 shared afterstate policy/value network
- batched PPO는 안정적인 첫 후보지만 확정 알고리즘이 아니다.
- 대안으로 off-policy value learning, search imitation, noisy cross-entropy/ES를 같은 simulator-step과 wall-time 예산에서 비교한다.
- action 수가 state마다 달라지므로 legal afterstate mask와 permutation-safe batching을 사용한다.

알고리즘 선택 지표는 held-out cross-play strength, sample efficiency, wall-clock, inference p50/p95, peak memory, seed 간 분산이다. solo line score만으로 선택하지 않는다.

## 5. 보상 함수의 수학적 설계

### 5.1 원래 목적

player 1의 원래 보상은 다음과 같다.

```text
r_1(s,a,s') = +1  if player 1 wins the round
             = -1  if player 1 loses the round
             =  0  otherwise
r_2 = -r_1
```

공격량, hole 감소, 생존 시간, APM을 독립적인 direct reward로 더하지 않는다. 그렇게 하면 공격 farming, 무의미한 stalling, 승률을 낮추는 suicidal spike가 보상상 유리해질 수 있다.

### 5.2 허용하는 dense shaping

player `i`의 bounded potential을 `Phi_i(s)`라 하고 다음 형태만 허용한다.

```text
F_i(s,s') = gamma * Phi_i(s') - Phi_i(s)
r'_i      = r_i + alpha * F_i
```

strict zero-sum을 유지하려면 player swap 연산 `swap(s)`에 대해 다음을 만족시킨다.

```text
Phi_2(s) = -Phi_1(s) = Phi_1(swap(s))
Phi_i(terminal) = 0
```

예를 들어 player별 normalized feature vector를 `psi_i(s)`라 할 때

```text
Phi_1(s) = clip(w^T(psi_1(s) - psi_2(s)), -1, 1)
Phi_2(s) = -Phi_1(s)
```

로 둘 수 있다. 여기서 `w`는 임의 직감으로 확정하지 않고 축약 game 검증과 ablation 대상으로 둔다.

### 5.3 무엇이 증명되고 무엇이 증명되지 않는가

Ng 등의 potential-based shaping 정리는 MDP에서 최적 policy 불변 조건을 제공한다. 이 프로젝트처럼 2인 game에는 stochastic game에서 Nash equilibrium의 불변성을 다룬 Lu·Schwartz·Givigi의 결과를 직접 근거로 사용한다.

`gamma = 1`, 고정 초기 상태, terminal potential 0인 finite episode라면 shaping 합은

```text
sum_t [Phi(s_{t+1}) - Phi(s_t)] = -Phi(s_0)
```

로 telescoping되어 policy와 무관한 상수가 된다. `gamma < 1`은 할인된 potential 형식과 정리의 가정을 맞춘다.

이것이 보장하는 것은 정확히 모델링된 game에서 목적/균형을 바꾸지 않는다는 점이다. neural approximation, finite sample, clipping, asynchronous rollout, 잘못된 terminal 처리에서 학습이 빨라지거나 안정적이라는 보장은 없다.

### 5.4 feature별 검증

각 potential feature는 다음 네 단계를 통과해야 한다.

1. **형식 검증:** 범위, 단위, normalization, swap antisymmetry와 terminal zero를 property test로 확인한다.
2. **축약 exact game:** 작은 board·짧은 bag·작은 garbage state를 전수 열거하고 shaping 전후의 최적 policy/Nash equilibrium 집합이 같은지 계산한다.
3. **효과 방향 실험:** 한 feature씩 넣고 뺀 paired-seed 실험으로 gradient variance, learning curve AUC, 최종 cross-play를 측정한다.
4. **채택 기준:** 여러 seed의 confidence interval에서 wall-time/sample 효율을 개선하고 held-out exploitability proxy를 악화시키지 않을 때만 유지한다.

`alpha`와 `w`도 reward 정의의 일부이므로 실험 기록과 hash에 포함한다. 최종 모델 비교에서는 shaping이 없는 terminal-only run을 반드시 유지한다.

## 6. 휴리스틱 기록 기반 초기화

RL을 random initialization에서 바로 시작하지 않는다. 여러 linear/search teacher가 생성한 records로 afterstate scorer와 value head를 먼저 학습한다.

- chosen action뿐 아니라 모든 legal candidate의 score, rank와 margin을 저장한다.
- teacher가 자체 방문한 state의 behavior cloning 뒤, learner rollout state에 teacher를 다시 질의하는 DAgger형 aggregation을 수행한다.
- teacher ID·style·rating·search budget을 보존해 약하거나 모호한 label을 구분한다.
- solo 기록은 board representation bootstrap에만 사용하고, final initialization은 attack/garbage/opponent context가 있는 versus 기록으로 만든다.
- terminal RL fine-tuning과 imitation auxiliary loss의 유지·anneal·제거를 비교해 teacher ceiling을 넘을 경로를 보존한다.

offline accuracy가 높아도 closed-loop 대국이 약할 수 있으므로 checkpoint 승격은 held-out opponent/seed 대국으로 결정한다. 상세 schema와 실험은 `IMITATION_BOOTSTRAP_RESEARCH_KO.md`를 따른다.

## 7. self-play와 상대 population

단일 최신 checkpoint끼리만 반복하면 순환 전략, 망각과 과적합이 생길 수 있다. 제한된 자원에서는 PSRO와 league training의 아이디어를 축소해 사용한다.

- current policy, 과거 checkpoint, scripted baseline, 약점 exploit용 policy로 bounded pool을 구성한다.
- payoff matrix를 cache하고 불확실성이 크거나 약한 matchup을 우선 sampling한다.
- pool 크기와 historical checkpoint 수를 고정해 비용 폭증을 막는다.
- evaluation 상대와 seed set은 학습에서 분리한다.
- candidate promotion은 population cross-play와 held-out suite를 모두 통과해야 한다.

완전한 PSRO는 policy 수에 따라 payoff 평가 비용이 커지므로 처음부터 구현하지 않는다. AlphaStar 규모의 league를 재현하는 것이 아니라 다양성·과거 상대·exploiter라는 개념만 예산에 맞게 적용한다.

## 8. 평가 설계

### 7.1 주지표

- held-out opponent population에 대한 paired match win rate와 95% interval
- 전체 cross-play payoff matrix와 style cluster별 최저 성능
- fixed baseline 대비 Elo-like rating과 불확실성
- inference latency p50/p95, nodes per move, environment steps/s
- 학습 wall-time, sample 수, peak memory, 가능하면 energy

### 7.2 보조지표

- APP/APL/PPS, cancellation efficiency, downstack latency
- B2B/Surge conversion과 break timing
- danger-state 생존율과 terminal 원인 분포
- illegal/unreachable action rate는 항상 0이어야 함

self-play 평균 reward가 0에 가깝다는 것은 대칭 환경의 자연스러운 결과일 수 있으므로 강함의 지표가 아니다. score·APM 같은 보조지표도 승률을 대신하지 않는다.

### 7.3 실험 공정성

- 동일 mechanics hash, seed pairs, opponent order와 compute budget
- side swap과 mirrored garbage/piece seed
- 최소 5 training seeds와 사전에 정한 match 수
- effect size, confidence interval, raw result artifact 공개
- model parameter 수뿐 아니라 simulator throughput과 총 wall-time을 함께 보고

## 9. 제한 자원 최적화

우선순위는 simulator가 학습보다 느리지 않도록 만드는 것이다.

1. Rust bitboard와 precomputed piece/kick mask
2. rendering·allocation 없는 deterministic step loop
3. many-arena batching과 structure-of-arrays 형태의 rollout buffer
4. legal afterstate와 reachability 결과 cache
5. Python 경계 호출 수를 줄이는 large-batch FFI
6. mixed precision은 numerical equivalence와 throughput이 실제 개선될 때만 사용
7. profile-guided optimization 전에 flamegraph/criterion 기준선 확보
8. replay/fixture mode와 fast training mode가 같은 state-transition code를 공유

GPU utilization만 높이는 것은 목표가 아니다. `matches per joule` 또는 `validated environment steps per second`와 최종 strength를 함께 최적화한다.

## 10. 단계별 결정 게이트

| 게이트 | 질문 | 통과 증거 |
|---|---|---|
| R0 | simulator가 정확한가 | mechanics conformance suite 100% |
| R1 | linear feature가 작동하는가 | scripted/linear 기준선 재현성과 paired 결과 |
| R1.5 | imitation bootstrap이 유효한가 | learner-state aggregation 뒤 held-out closed-loop 우위 |
| R2 | MLP가 비용 대비 낫나 | 동일 wall-time에서 M0보다 held-out 우위 |
| R3 | spatial encoder가 필요한가 | M1 대비 strength-latency Pareto 우위 |
| R4 | shaping이 유효한가 | exact-game 불변성 + multi-seed learning 효율 개선 |
| R5 | self-play가 일반화하나 | unseen style/opponent 및 adversarial pool 통과 |
| R6 | search가 값을 더하나 | node/latency budget 내 population 우위 |

## 11. 주요 출처

- [Scherrer et al., Approximate Modified Policy Iteration and its Application to the Game of Tetris, JMLR 2015](https://jmlr.org/papers/v16/scherrer15a.html)
- [Algorta & Şimşek, The Game of Tetris in Machine Learning, 2019](https://arxiv.org/abs/1905.01652)
- [Chen et al., An Efficient Bitboard-Based Environment for Reinforcement Learning in Tetris, 2026 preprint](https://arxiv.org/abs/2603.26765)
- [Ng, Harada & Russell, Policy Invariance under Reward Transformations, 1999](https://ai.stanford.edu/~ang/papers/shaping-icml99.pdf)
- [Lu, Schwartz & Givigi, Policy Invariance under Reward Transformations for General-Sum Stochastic Games, 2014](https://arxiv.org/abs/1401.3907)
- [Devlin & Kudenko, Theoretical Considerations of Potential-Based Reward Shaping for Multi-Agent Systems, 2011](https://www.ifaamas.org/Proceedings/aamas2011/papers/D1_G45.pdf)
- [Lanctot et al., A Unified Game-Theoretic Approach to Multiagent Reinforcement Learning, PSRO, 2017](https://mlanctot.info/files/papers/nips17-psro.pdf)
- [Lanctot et al., OpenSpiel, 2019](https://arxiv.org/abs/1908.09453)
- [Vinyals et al., Grandmaster level in StarCraft II using multi-agent reinforcement learning, 2019](https://doi.org/10.1038/s41586-019-1724-z)
- [Stanford CS224R student project, Reinforcement Learning in Tetris with Multi-Agent Systems, 2025](https://cs224r.stanford.edu/projects/pdfs/224R_Paper__1_.pdf)
- [Zhang, Cai & Nebel, Playing Tetris Using Learning by Imitation, 2010](https://www.eurosis.org/cms/files/proceedings_full/GAMEON2010.deel1_2.11.10.rdo.pdf)
- [Ross, Gordon & Bagnell, DAgger, 2011](https://proceedings.mlr.press/v15/ross11a.html)
- [Hester et al., Deep Q-learning from Demonstrations, AAAI 2018](https://ojs.aaai.org/index.php/AAAI/article/view/11757)
- [Beliaev et al., Imitation Learning by Estimating Expertise of Demonstrators, ICML 2022](https://proceedings.mlr.press/v162/beliaev22a.html)
- [Anthony, Tian & Barber, Expert Iteration](https://arxiv.org/abs/1705.08439)

위 Stanford CS224R 자료는 non-peer-reviewed custom environment 결과이므로 multi-agent imitation의 탐색 가설로만 사용한다.
