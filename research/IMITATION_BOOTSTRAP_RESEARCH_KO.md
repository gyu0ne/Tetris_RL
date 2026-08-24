# 휴리스틱 기록과 모방학습 조사

조사일: `2026-08-24`

## 결론

휴리스틱 알고리즘으로 많은 기록을 만든 뒤 기본 모델을 구성하는 방향은 제한된 연산 자원에서 합리적이다. 특히 legal afterstate 열거와 강한 search/linear evaluator가 이미 있는 Tetris에서는 random policy에서 sparse terminal reward만으로 시작하는 비용을 줄일 수 있다.

그러나 **teacher가 선택한 행동만 대량 복제하는 방식은 충분하지 않다.** 최종 권고는 다음 세 요소의 결합이다.

1. heuristic/search teacher의 후보 전체 score를 이용한 policy/value pretraining
2. learner가 방문한 state를 다시 teacher가 label하는 dataset aggregation
3. 실제 terminal 승패 목적의 self-play RL fine-tuning

## 직접 관련 Tetris 연구

Zhang·Cai·Nebel의 2010년 연구는 Tetris placement imitation을 classification 문제로 바꾸고 서로 다른 인간 player의 style을 학습할 수 있음을 보고했다. 이는 “placement 후보 중 teacher 행동을 분류한다”는 기본 형식의 직접 근거다. 하지만 오래된 별도 Tetris platform, 인간 demonstration과 당시 bot을 대상으로 하므로 현재 TL 1 대 1 성능을 보장하지 않는다.

Algorta와 Şimşek의 survey는 강한 Tetris 연구에서 linear afterstate evaluator와 classification-based policy iteration이 중요한 계보였음을 정리한다. Scherrer 등의 CBMPI는 강한 기존 policy가 만든 trajectory state에서 rollout 비교로 policy를 개선하고 비교적 적은 sample로 경쟁력 있는 결과를 보였다. 즉 trajectory를 단순 복사하는 것보다 candidate quality를 비교하는 label이 더 유용할 수 있다.

## 일반 imitation 연구의 적용

### Behavior cloning의 한계와 DAgger

Behavior cloning은 teacher가 방문한 상태 분포에서 supervised loss를 최소화한다. learner가 한 번 실수하면 teacher dataset에 드문 상태로 이동하고 추가 오류가 누적될 수 있다. DAgger는 learner를 실제로 rollout한 뒤 그 상태에 expert label을 모아 dataset을 반복 확장한다.

이 프로젝트에서는 teacher가 자동 heuristic/search이므로 인간에게 새 label을 요청하는 비용이 없다. 따라서 다음 변형이 가능하다.

```text
D_0 = 여러 teacher가 자체 rollout한 기록
pi_0 = BC(D_0)
D_k = D_(k-1) + teacher_label(states visited by pi_(k-1))
pi_k = train(D_k)
```

learner와 teacher를 확률적으로 섞어 움직일 필요 없이, local engine에서 learner rollout을 별도로 수행하고 모든 learner state에 teacher를 offline 질의할 수 있다.

### Demonstration과 RL 결합

DQfD는 expert demonstration을 supervised margin loss와 TD update에 함께 사용하고 prioritized replay로 demonstration을 유지한다. 이 프로젝트는 state마다 action 수가 다른 afterstate policy라서 DQfD 네트워크를 그대로 사용하지 않는다. 대신 다음 원리를 채택한다.

- BC로 actor/scorer와 value head를 초기화
- demonstration transition을 RL replay/auxiliary loss에 일정 비율 유지하는 실험
- teacher보다 강해질 수 있도록 terminal RL update가 teacher label을 영구적으로 압도하지 않게 loss weight를 anneal

AlphaStar도 큰 차이는 있지만 human replay supervised initialization 뒤 multi-agent RL을 수행한 사례다. 이 프로젝트는 human data 대신 rules-correct heuristic/search data를 사용한다.

### 여러 teacher의 품질

teacher가 여러 개면 표준 BC는 모두 같은 품질로 취급한다. Beliaev 등은 demonstrator의 전문성이 상태마다 다를 수 있으며 이를 구분하는 것이 성능에 도움이 될 수 있음을 보였다. 따라서 데이터에 teacher ID만 넣는 것이 아니라 다음을 함께 저장한다.

- teacher의 고정 evaluation rating과 style
- search depth/node/time budget
- top-1/top-2 score margin
- 후보별 normalized score/rank
- terminal 결과와 상대 ID

초기 구현은 복잡한 latent expertise model보다 명시적인 rating/margin 기반 sampling을 사용하고, 필요할 때만 학습된 expertise weighting을 실험한다.

## Tetris 1 대 1용 teacher 구성

### teacher ladder

1. linear Dellacherie/Thiery 계열 board evaluator
2. versus feature를 추가한 linear evaluator
3. 다양한 weight/style을 가진 heuristic ensemble
4. 1~N ply beam/search teacher
5. 과거 가장 강한 policy를 search prior로 쓰는 Expert-Iteration형 teacher

하나의 최고 teacher만 쓰지 않는 이유는 동일한 tie-breaking과 동일한 blind spot을 dataset 전체에 복제하지 않기 위해서다.

### 필수 versus label

- 공격과 cancel 후의 실제 state
- incoming packet와 activation timing
- B2B/Surge/opener state 변화
- 상대 danger와 예상 counterattack
- terminal outcome 또는 제한 horizon search value

solo line-clear heuristic만으로 만든 모델은 survival/flat-stack은 배울 수 있어도 timing, spike와 downstack trade-off를 배우지 못한다.

## 권장 record schema

각 decision record는 최소 다음 필드를 가진다.

```text
schema_version, rules_hash, engine_revision
match_id, seed_pair, ply, player_side
observation_hash, packed_public_observation
legal_candidates[]:
  action_token, afterstate_hash, path_token
  teacher_score, rank, immediate_events
chosen_action_token
teacher_id, teacher_config_hash, style, node_budget, strength_snapshot
terminal_outcome, remaining_horizon
```

candidate 전체를 저장하면 top action classification, score regression, pairwise ranking, temperature distillation을 같은 shard에서 비교할 수 있다. storage가 병목이면 packed fixed-width core와 variable candidate side table을 분리한다.

## 손실 후보

teacher의 후보 score를 `q_j`라 하면 temperature `tau`로 soft target을 만들 수 있다.

```text
p_teacher(j|s) = softmax(q_j / tau)
L_policy = -sum_j p_teacher(j|s) log pi_theta(j|s)
```

near-tie 상태에서는 큰 `tau` 또는 낮은 sample weight를 사용하고, 명백한 차이는 높은 weight로 학습한다. 값 head는 teacher heuristic 자체가 아니라 search return 또는 terminal outcome을 우선 label로 사용한다. heuristic scale을 value truth로 착각하지 않는다.

## 검증 실험

| 실험 | 비교 | 질문 |
|---|---|---|
| I0 | random init vs BC | 초기 closed-loop strength가 실제 개선되는가 |
| I1 | chosen-only vs full-score distillation | 후보 점수가 sample efficiency를 높이는가 |
| I2 | single teacher vs ensemble | style/상성 일반화가 개선되는가 |
| I3 | BC vs BC+aggregation | learner distribution 오류가 줄어드는가 |
| I4 | BC only vs BC→RL | terminal RL이 teacher ceiling을 넘는가 |
| I5 | demo loss 유지/anneal/제거 | RL 안정성과 최종 강도의 trade-off는 무엇인가 |

모든 비교는 같은 mechanics hash, seed pairs, environment steps, wall-time과 inference budget을 사용한다.

## 주요 출처

- [Zhang, Cai & Nebel, Playing Tetris Using Learning by Imitation, 2010](https://www.eurosis.org/cms/files/proceedings_full/GAMEON2010.deel1_2.11.10.rdo.pdf)
- [Ross, Gordon & Bagnell, DAgger, AISTATS 2011](https://proceedings.mlr.press/v15/ross11a.html)
- [Scherrer et al., Approximate Modified Policy Iteration and its Application to Tetris, JMLR 2015](https://jmlr.org/papers/v16/scherrer15a.html)
- [Algorta & Şimşek, The Game of Tetris in Machine Learning, 2019](https://arxiv.org/abs/1905.01652)
- [Hester et al., Deep Q-learning from Demonstrations, AAAI 2018](https://ojs.aaai.org/index.php/AAAI/article/view/11757)
- [Beliaev et al., Imitation Learning by Estimating Expertise of Demonstrators, ICML 2022](https://proceedings.mlr.press/v162/beliaev22a.html)
- [Anthony, Tian & Barber, Expert Iteration, 2017/2018](https://arxiv.org/abs/1705.08439)
- [Vinyals et al., AlphaStar, Nature 2019](https://doi.org/10.1038/s41586-019-1724-z)
