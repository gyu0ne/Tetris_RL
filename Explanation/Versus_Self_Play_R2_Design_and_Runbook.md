# 1대1 자기대전 r2 설계 근거와 실행법

## 1. 결론

r2의 목표는 공격량을 최대화하는 봇이 아니라 **승률을 최대화하면서 솔로 생존 능력을 잃지 않는 봇**이다. 공격, 안정적인 줄 지우기, 방어는 별도 점수를 모으는 세 목표가 아니라 승리 정책이 갖춰야 할 행동 특성으로 취급한다.

r1에서는 학습 모델의 공격량이 reference보다 높았지만 대국 결과는 항상 더 좋지 않았다. update 200·230·310 사이에는 순환 우위도 관측됐다. 따라서 `attack/piece`만 보상하면 공격을 많이 만들고도 위험한 stack 때문에 지는 정책을 더 강화할 수 있다. r2는 승패 외의 공격 보너스를 reward에 직접 넣지 않는다.

## 2. 목적함수와 시간 척도

기본 보상은 종료 승패뿐이다.

```text
win = +1, loss = -1, draw = 0, non-terminal = 0
```

PPO는 이 보상의 할인 return을 최적화한다. r1 대국은 승패가 대략 1,400~1,700수에서 갈리는 경우가 많았는데 `gamma=0.997`이면 1,400수 뒤 신호가 다음처럼 거의 사라진다.

```text
0.997^1400 = 0.014901
```

r2는 `gamma=0.9995`를 사용한다.

```text
0.9995^1400 = 0.496498
할인 반감기 = log(0.5) / log(0.9995) = 1385.95수
```

GAE는 `lambda=0.995`다. 지수 trace의 대략적인 유효 길이는 다음과 같다.

```text
r1: 1 / (1 - 0.997*0.95)   = 18.92수
r2: 1 / (1 - 0.9995*0.995) = 181.90수
```

500수 떨어진 advantage 항도 `(0.9995*0.995)^500 = 0.06352`가 남는다. 분산 증가를 완화하기 위해 32개 병렬 경기에서 512 step을 모은 뒤 PPO update를 수행한다. 이 값이 이론적으로 유일한 최적값이라는 뜻은 아니며, r1의 실제 경기 길이를 기준으로 terminal 신호가 사라지지 않게 정한 사전 선언값이다.

## 3. 보조 보상: 목표를 바꾸지 않는 형태만 사용

보조 보상은 다음 potential difference뿐이다.

```text
r = terminal_outcome + 0.05 * (0.9995*Phi(next) - Phi(current))
Phi(terminal) = 0
```

`Phi`는 양쪽 최대 높이, 구멍, pending/ready garbage, combo, B2B의 정규화 차이다. 가중치는 차례로 `0.25, 0.25, 0.15, 0.25, 0.05, 0.05`다. 즉 높이·구멍으로 안정성을, ready garbage로 즉시 방어 압력을, pending garbage로 장기 압력을 표현한다.

모든 항은 `[-1,1]`로 자르고 가중치 합이 1이므로 `|Phi|<=1`이다. player를 맞바꾸면 `Phi` 부호도 바뀐다. 한 transition의 shaping 절댓값 상한은 다음과 같다.

```text
0.05 * (1 + 0.9995) = 0.099975
```

할인된 shaping 합은 중간 항이 소거되어 같은 초기 상태에서는 정책이 바꿀 수 없는 경계항만 남는다. Potential-based shaping이 MDP의 최적 정책을 보존한다는 Ng·Harada·Russell의 결과와, stochastic game의 Nash equilibrium 보존 결과에 맞춘 형태다. 공격 보너스나 T-spin 보너스를 직접 넣지 않은 이유도 이 보장을 깨지 않기 위해서다.

## 4. 모델: 작은 크기로 상호작용을 표현

r1 actor는 `solo_score + context_score`였다. 이 구조는 같은 board feature의 의미가 garbage 상황에 따라 달라지는 상호작용을 표현하기 어렵다. r2는 솔로 scorer를 base로 유지하고 다음 76개 후보 특징을 한 번에 보는 residual MLP를 더한다.

- 기존 솔로 afterstate 특징 10개
- 공격·상쇄·양쪽 garbage/stack 문맥 10개
- 후보 착지 후 열 높이 10개
- 후보 착지 후 열별 구멍 10개
- hold 사용 여부 1개
- 현재, hold, preview 3개 조각의 7종 one-hot 35개

```text
policy logit = frozen-start solo_score + residual(76→64→32→1)
```

residual 마지막 층을 0으로 초기화하므로 첫 행동 분포는 승격된 솔로 모델과 정확히 같다. 이후에는 `높은 stack + ready garbage`, `구멍 구조 + 다음 조각`, `공격 후보 + 상쇄 가능량` 같은 결합 조건을 학습할 수 있다.

critic은 양쪽의 전역 특징, 10열 높이, 10열 구멍, 조각 문맥을 모두 사용한다. 같은 network에 `(own, opponent)`와 `(opponent, own)`을 넣은 차의 절반을 value로 사용해 `V(swap(s))=-V(s)`를 정확히 강제한다.

전체 모델은 14,883 parameter다. float32 weight만 계산하면 약 58 KiB라서 모델 크기보다 Rust 후보 열거와 장기 대국 simulation이 실행 시간의 대부분을 차지한다.

## 5. 솔로 능력 보존과 공격 학습

솔로 trunk의 learning rate는 새 residual/value head의 0.1배다. 또한 고정된 솔로 teacher와 learner의 후보 분포 차이를 다음 KL로 제한한다.

```text
L = L_PPO + 0.5*L_value - beta_entropy*H_normalized
  + beta_teacher*KL(teacher || learner)

beta_teacher: update 0의 0.02 → update 500의 0.001
```

이는 teacher를 영원히 복제하는 행동 cloning이 아니다. Schmitt 등의 kickstarting처럼 teacher loss를 보조항으로 두고 감소시켜 초기 안전한 줄 지우기를 보존하면서 terminal 승패가 더 좋은 공격·방어 정책으로 벗어날 수 있게 한다.

Tetris 연구에서 compact board features를 사용하는 policy search가 전통적 value approximation보다 강한 결과를 냈다는 보고가 있고, 좋은 정책이 좋은 value보다 표현하기 쉽다는 실험도 있다. 현재 CPU 후보 수가 평균 약 68개이므로 후보마다 CNN을 실행하기보다 열 구조·구멍·조각 정보를 가진 작은 공동 scorer를 먼저 선택했다. T-spin 생성은 보상으로 강제하지 않는다. T-spin이 실제 승률을 높이면 공격/상쇄/garbage 문맥과 조각 preview를 통해 terminal objective가 선택하도록 둔다.

## 6. 상대 리그

경기 구성은 다음과 같다.

- 35%: 현재 learner끼리 자기대전
- 50%: 과거 모델 PFSP
- 15%: 고정 솔로 bootstrap

과거 풀은 최근 8개만 쓰지 않는다. 전체 snapshot 이력에서 처음·중간·최신 전략이 남도록 최대 32개를 층화한다. 각 상대에 대한 learner score를 Laplace 방식으로 완화한다.

```text
p = (wins + 0.5*draws + 1) / (games + 2)
weight = max(0.05, (1-p)^1)
```

learner가 잘 못 이기는 상대일수록 더 자주 뽑지만 모든 상대에 최소 확률을 남긴다. 이는 AlphaStar가 자기대전의 망각과 순환을 줄이기 위해 사용한 league/PFSP 원칙을 현재 계산량에 맞게 축소한 것이다. 한 경기에 배정된 상대 checkpoint와 learner 진영은 terminal까지 바뀌지 않으며 resume checkpoint에도 저장된다.

## 7. 실행과 중간 저장

PowerShell에서 다음 명령으로 새 r2를 시작한다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile max -Hours 24
```

자원 프로필은 Rust 후보 생성과 PyTorch의 작은 MLP에 같은 thread 수를 주지 않는다. `max`는 `Rayon 12 / PyTorch 2`, `balanced`는 `6 / 2`, `light`는 `2 / 1`이다. 실제 측정에서 PyTorch 12-thread는 작은 minibatch의 scheduling 비용 때문에 2-thread보다 느렸다.

기본 출력은 `checkpoints/versus-selfplay-r2`다. r1과 구조가 다르므로 r1 snapshot을 `-InitializeFrom`으로 주면 명시적으로 거부한다. 승격된 솔로 bootstrap에서 새로 시작하는 것이 정상이다.

중단 후 재개:

```powershell
./scripts/run-versus-selfplay.ps1 `
  -ResourceProfile max `
  -Hours 24 `
  -OutputDir checkpoints/versus-selfplay-r2 `
  -Resume
```

저장 파일:

```text
latest.pt         매 update의 optimizer·환경·상대·league 통계 포함 정확 재개점
latest-model.pt   매 update의 즉시 사용 가능한 추론 모델
model.pt          정상 종료 시 최종 추론 모델
reference-model.pt 시작 정책
snapshots/        10 update 간격 progress와 추론 모델
```

강제 중지로 `finally`가 실행되지 않아도 마지막 완료 update의 `latest.pt`와 `latest-model.pt`는 남는다.

## 8. 로그 해석과 최종 선택

공격:

- `attack_per_piece`: 만든 총 공격
- `outgoing_attack_per_piece`: 자기 garbage 상쇄 뒤 실제 송신
- `cancelled_attack_per_piece`: 방어에 사용한 공격
- `tetris_per_100`, `t_spin_*_per_100`: 고급 공격 빈도

안정성·방어:

- `mean_max_height`, `mean_holes`: stack 안정성
- `mean_pending_garbage`, `mean_ready_garbage`: 받고 있는 압력
- `danger_rate`: 자기 최대 높이가 16 이상인 결정 비율
- `bootstrap_rolling_10_score`, `historical_rolling_10_score`: 고정 상대 실전 성과

학습 건전성:

- `explained_variance`: critic 설명력
- `approximate_kl`, `clip_fraction`, `gradient_norm`: PPO update 크기
- `rollout_normalized_entropy`, `rollout_mean_max_probability`: 후보 수를 보정한 탐색 정도
- `kickstart_kl`, `kickstart_loss_contribution`: 솔로 teacher에서 벗어난 정도와 실제 loss 영향
- `learner_terminal_*`, `terminal_transition_rate`: 학습자에게 실제 승패 terminal이 전달된 횟수와 비율
- `shaping_reward_nonzero_rate`, `shaping_reward_mean_abs`: 플레이 중 potential 보상의 발생률과 크기
- `shaping_*_mean_abs`: stack·holes·garbage·combo·B2B별 조밀 보상 기여
- `rollout_environment_seconds`, `rollout_policy_seconds`, `ppo_seconds`: 후보 생성·정책 수집·학습 구간별 병목

최종 모델은 최신 snapshot이나 공격량 하나로 정하지 않는다. reference 및 여러 과거 snapshot과 같은 seed·좌우 교대 대국을 수행하고, 승률 Wilson 95% 구간과 completion rate를 우선한다. 동률권에서는 outgoing attack이 높고 danger/holes가 낮은 모델을 고른다. 8/12/15 frame cadence에서도 순위가 유지되는지 확인해야 cadence에 과적합되지 않았다고 본다.

## 9. 근거 자료

모두 2026-08-28에 열람했다.

- Schulman et al., [Proximal Policy Optimization Algorithms](https://arxiv.org/abs/1707.06347), 2017.
- Schulman et al., [High-Dimensional Continuous Control Using Generalized Advantage Estimation](https://arxiv.org/abs/1506.02438), 2015.
- Ng, Harada, Russell, [Policy Invariance under Reward Transformations](https://ai.stanford.edu/~ang/papers/shaping-icml99.pdf), ICML 1999.
- Devlin, Kudenko, [Policy Invariance under Reward Transformations for General-Sum Stochastic Games](https://js.aaai.org/Papers/JAIR/Vol41/JAIR4112.pdf), JAIR 2011.
- Schmitt et al., [Kickstarting Deep Reinforcement Learning](https://arxiv.org/abs/1803.03835), 2018.
- Thiery, Scherrer, [Improvements on Learning Tetris with Cross Entropy](https://journals.sagepub.com/doi/pdf/10.3233/ICG-2009-32104), 2009.
- Gabillon, Ghavamzadeh, Scherrer, [Approximate Dynamic Programming Finally Performs Well in the Game of Tetris](https://proceedings.neurips.cc/paper_files/paper/2013/hash/7504adad8bb96320eb3afdd4df6e1f60-Abstract.html), NeurIPS 2013.
- Vinyals et al., [Grandmaster level in StarCraft II using multi-agent reinforcement learning](https://www.nature.com/articles/s41586-019-1724-z), Nature 2019.

## 10. 현재 보장과 남은 실험

구현·단위 테스트·한 update 스모크는 통과했다. 후보 엔진 마이크로벤치는 2-thread에서 15.923초에서 0.426초로 단축됐고, 첫 update에서는 terminal이 0이어도 학습자 transition의 약 96.3%에 potential shaping이 들어갔다. 상세 측정과 로그 해석은 [1대1 학습 성능 최적화와 보상 진단](./Versus_Training_Performance_and_Reward_Diagnostics.md)에 기록했다. 이것은 학습기가 의도대로 계산된다는 증거이지 r2가 이미 r1보다 강하다는 결과는 아니다. 실제 24시간 run, snapshot league 평가, shaping/KL ablation, 8/12/15 cadence 비교가 끝나야 최종 champion을 선택할 수 있다.
