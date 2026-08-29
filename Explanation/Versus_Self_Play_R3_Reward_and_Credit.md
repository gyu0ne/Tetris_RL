# 1대1 자기대전 r3 보상·크레딧 설계와 실행

## 목적

r3는 긴 대전에서 승패 신호가 너무 멀리 떨어지는 문제와, 안전한 솔로 정책이 공격 기회를 거의 선택하지 않는 문제를 함께 다룬다. 최종 목표는 계속 `승리 +1 / 패배 -1 / 무승부 0`이다. 줄 삭제, 공격, T-spin 자체를 영구 보상으로 더하지 않는다.

새 학습은 기존 r2와 분리된 `checkpoints/versus-selfplay-r3/`에 저장한다. r2 체크포인트와 진행 상태는 수정하지 않는다.

## 1. 잠재함수 기반 전술 준비도

기존 구조 잠재함수는 높이, 구멍, 대기·준비 가비지, 콤보, B2B를 정규화한 플레이어 상대값이다. r3는 현재 합법 후보 전체에서 다음 세 준비도를 추가한다.

- 공격 준비도: 후보 중 최대 cancellation 후 outgoing attack을 8줄로 나누고 `[0, 1]`로 제한한다.
- 방어 준비도: 후보 중 최대 `total attack - outgoing attack`, 즉 현재 가비지를 상쇄할 수 있는 양을 8줄로 나누고 제한한다.
- Full T-spin 준비도: Full T-spin 후보가 낼 수 있는 최대 total attack을 8줄로 나누고 제한한다.

각 플레이어의 전술 준비도는 `0.4×공격 + 0.4×방어 + 0.2×Full T-spin`이다. 실제 전술 성분은 항상 `내 준비도 - 상대 준비도`로 만들기 때문에 플레이어를 바꾸면 부호가 정확히 반전된다.

전체 잠재함수는 다음과 같다.

```text
Phi(s) = 0.75 * Phi_structural(s) + 0.25 * Phi_tactical(s)
F(s,s') = 0.05 * (0.9995 * Phi(s') - Phi(s))
Phi(terminal) = 0
```

두 부분 모두 절댓값이 1 이하이므로 전체 `|Phi| <= 1`이고, 한 전이의 shaping 절댓값 상한은 `0.05×(1+0.9995)=0.099975`다. 할인 누적하면 중간 항이 상쇄되는 potential-based shaping이므로 공격 보너스를 직접 더하는 방식과 달리 최종 승패 목적을 바꾸지 않는다.

이 전술 잠재함수는 T-spin을 실행한 순간에 점수를 주는 것이 아니다. 좋은 공격·상쇄·Full T-spin 선택지를 만들 수 있는 상태로 이동했을 때 크레딧을 앞당긴다. 실제로 그 기회를 실행할지는 승패와 아래의 임시 커리큘럼이 학습시킨다.

## 2. 안전성 제한 전술 커리큘럼

솔로 교사 점수는 절대 크기가 매우 크므로 작은 전술 값을 그대로 더하면 아무 변화가 없고, 반대로 전술 값만으로 목표분포를 만들면 안전한 쌓기를 무시한다. r3는 각 상태 안에서 교사 logits를 `[0,1]`로 min-max 정규화한다.

```text
teacher_unit(a) = (teacher(a) - min teacher) / (max teacher - min teacher)
tactical(a) = 0.4*outgoing/8 + 0.4*cancel/8 + 0.2*full_t_spin_attack/8
target(a) = argmax [teacher_unit(a) + tactical(a)]
```

각 분수는 `[0,1]`로 제한한다. 따라서 전술 이득이 교사 기준 안전성 손실보다 클 때만 목표 행동이 교사 1순위에서 바뀐다. 목표 행동에 대한 cross-entropy 계수는 update 0의 `0.0001`에서 update 300의 `0`까지 선형 감소한다. 이후 최적화에는 승패 PPO, 잠재함수 shaping, 감소 후에도 남는 솔로 kickstart만 존재한다.

첫 32경기×512-step 스모크에서 전술 목표가 교사 1순위와 달라진 비율은 `0.3497%`였다. 전술 cross-entropy의 손실 기여는 `5.76e-5`로 normalized entropy 항 `-4.84e-5`와 같은 크기였고, PPO gradient를 압도하지 않았다. 순수 전술분포 KL의 초기 기여 `0.439`는 과도하여 폐기했다.

## 3. 긴 승패 크레딧

`gamma=0.9995`는 유지하고 GAE lambda를 `0.995`에서 `0.999`로 높였다. trace 계수는 `gamma×lambda=0.9985005`이며 이론적 유효 길이는 약 `667` decision이다. terminal 신호의 trace 비율은 100 decision 전 약 `0.861`, 250 전 약 `0.687`, 500 전 약 `0.472`다. r2의 500 decision 전 비율 약 `0.064`보다 긴 판의 승패를 훨씬 강하게 전달한다.

분산 증가를 감시하기 위해 로그는 terminal reward trace와 shaping trace를 분리해 다음을 출력한다.

- `terminal_trace_mean_abs`, `terminal_trace_nonzero_rate`
- `shaping_trace_mean_abs`, `shaping_trace_nonzero_rate`
- `terminal_shaping_trace_cosine`
- 9개 shaping 성분별 signed mean과 mean absolute value
- `tactical_curriculum_cross_entropy`, 실제 loss contribution, `tactical_target_change_rate`

한 update에 끝난 경기가 없으면 terminal trace가 0인 것은 정상이다. 경기는 update 경계를 넘어 계속되므로 여러 update를 보고 terminal 비율과 승패 rolling score를 판단해야 한다.

## 4. 실행

새 r3를 처음 시작한다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile balanced -Hours 24
```

중단된 r3를 같은 설정으로 이어간다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile balanced -Hours 24 -Resume
```

최대 자원 프로필은 다음과 같다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile max -Hours 24
```

기존 r2를 재개해야 할 때만 보존된 legacy 스크립트를 사용한다.

```powershell
./scripts/run-versus-selfplay-r2.ps1 -ResourceProfile balanced -Hours 24 -Resume
```

r2의 `latest.pt`를 r3로 progress-resume하면 안 된다. 모델 가중치만 실험적으로 가져오려면 `-InitializeFrom`을 명시할 수 있지만, 기본 r3는 검증된 솔로 bootstrap에서 새로 시작한다.

## 5. 초기 로그 판정

최소 10 update 전에는 기술 빈도 하나로 중단하지 않는다. 다음 조건을 함께 본다.

- `loss`, `gradient_norm`, `approximate_kl`이 유한하고 폭주하지 않는가
- normalized entropy가 갑자기 1에 붙거나 0으로 붕괴하지 않는가
- danger, holes, max height가 지속 상승하지 않는가
- outgoing·cancelled attack이 유지 또는 증가하는가
- terminal trace가 실제 종료 이후 nonzero가 되는가
- 전술 loss contribution이 PPO policy/value 항을 장기간 압도하지 않는가

Full T-spin 빈도는 희소하므로 단기 목표가 아니다. 최종 모델은 snapshot별 held-out 승률, 공격, 생존·방어 지표를 함께 비교해 선택한다.
