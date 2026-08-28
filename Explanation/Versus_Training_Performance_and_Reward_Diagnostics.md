# 1대1 학습 성능 최적화와 보상 진단

## 1. 결론

기존 속도 병목은 테트리스 규칙 계산 자체가 아니라 후보 착지 하나를 평가할 때마다 같은 도달 가능 위치 탐색을 다시 실행하던 구조였다. 이를 제거하고 도달 경로 저장, afterstate 특징 계산, 신경망 추론과 PPO ragged 분포 계산을 묶어서 처리하도록 바꿨다.

보상은 승패만 들어가는 구조가 아니다. r2의 실제 transition 보상은 다음과 같다.

```text
r = terminal_outcome + 0.05 * (0.9995*Phi(next) - Phi(current))
```

`Phi`는 양쪽의 stack 높이, 구멍, pending garbage, ready garbage, combo, B2B 차이를 사용한다. 따라서 한 판이 끝나지 않은 수에도 보드 안정성·압박·방어 상태가 변하면 조밀 보상이 생긴다. 공격량, 줄 수, T-spin 횟수를 보상에 직접 더하지는 않는다. 그런 직접 보너스는 공격만 많이 하고 패배하는 정책을 강화할 수 있고 zero-sum 승리 목적을 바꿀 수 있기 때문이다.

## 2. 제거한 계산 병목

### 2.1 후보마다 반복되던 도달 탐색

이전 후보 생성은 한 상태에서 모든 도달 가능한 착지를 구한 뒤, 각 후보를 적용할 때 `lock_placement_deferred`가 도달 탐색을 다시 수행했다. 평균 후보가 약 68.5개였으므로 상태 하나에서 사실상 같은 탐색을 약 69번 반복했다.

현재는 최초 탐색에서 검증된 `GeometricPlacement`를 `preview_reachable_placement`에 전달한다. preview는 원본 게임을 바꾸지 않고 spin, line clear, perfect clear, garbage provenance와 compacted board를 계산하며 두 번째 탐색을 생략한다. preview 결과가 실제 mutating lock과 같은지는 모든 초기 reachable placement를 대상으로 단위 테스트한다.

### 2.2 경로와 board 특징

- BFS 노드마다 전체 이동 경로 `Vec`를 복사하지 않고 parent index와 마지막 movement만 저장한다.
- 최종 착지에 대해서만 경로를 역추적해 복원한다.
- 열 높이·구멍·row/column transition·well을 여러 번 cell scan하지 않고 10개 column bit mask에서 함께 계산한다.
- 후보의 piece context는 후보 루프 밖에서 한 번만 계산한다.

이 변경 뒤 동일한 16경기×20 step, Rayon 2-thread 마이크로벤치에서 다음 결과를 얻었다.

| 항목 | 변경 전 | 변경 후 | 배율 |
|---|---:|---:|---:|
| 20 step 엔진 시간 | 15.923초 | 0.426초 | 37.4배 |
| 착지 처리량 | 40.2/s | 1,502.9/s | 37.4배 |
| 후보 처리량 | 2,758/s | 103,086/s | 37.4배 |

### 2.3 Python 수집과 PPO

- 같은 actor를 쓰는 여러 경기의 후보를 한 번의 actor forward로 합친다.
- PPO에서 decision마다 `Categorical`과 `log_softmax`를 반복 생성하지 않고 variable-size segment의 log probability, entropy와 KL을 tensor 연산으로 계산한다.
- 고정 solo teacher의 log probability는 한 update에서 한 번 계산해 4 PPO epoch가 공유한다.
- 로그에 rollout 전체, Rust 환경, 정책 수집, value/reward, PPO 시간을 분리해 기록한다.

실제 32경기×512 step 첫 update는 머신 부하와 thread 설정에 따라 흔들리므로 마이크로벤치처럼 37배가 그대로 나오지는 않는다. 측정된 2-thread 실행은 약 133~139초였고, 기존 장기 실행의 약 284초/update보다 대략 2배 빠른 범위다.

## 3. Rust와 PyTorch thread를 분리한 이유

기존 실행 스크립트는 한 `Threads` 값을 Rayon 후보 생성과 PyTorch의 작은 MLP에 동시에 사용했다. 실제 측정에서 `Rust 12 / PyTorch 12`는 PPO만 262초가 걸렸고, `Rust 12 / PyTorch 2`에서는 PPO가 약 79초였다. 작은 network와 256-decision minibatch에서는 12-way PyTorch scheduling 비용이 계산 이득보다 컸다.

현재 자원 프로필은 학습 의미를 바꾸지 않고 runtime thread만 다음처럼 정한다.

| 프로필 | Rayon | PyTorch |
|---|---:|---:|
| light | 2 | 1 |
| balanced | 6 | 2 |
| max | 12 | 2 |

## 4. `2/32게임 종료`의 정확한 의미

로그의 `self_play_completed=2`는 전체 32경기 중 두 경기만 끝났다는 뜻이 아니다. 현재 learner끼리 배정된 경기 중 끝난 수만 뜻한다. 전체 종료 수는 `completed_matches`이고, bootstrap 및 historical 상대전 종료도 승패 학습에 포함된다.

새 로그는 다음을 별도로 표시한다.

- `completed_matches`: 해당 rollout에서 끝난 실제 경기 수
- `self_play_completed`: 그중 current-vs-current 경기 수
- `learner_terminal_decisions`: 학습자 transition 중 terminal에 닿은 수
- `learner_terminal_wins/losses/draws`: 학습자 관점의 terminal 결과
- `terminal_transition_rate`: 전체 학습자 decision에서 terminal transition 비율
- `nonzero_terminal_reward_rate`: 실제 ±1 승패가 붙은 transition 비율

새 run의 첫 update는 모든 경기가 빈 보드에서 시작하므로 512수 안에 종료가 0일 수도 있다. 경기는 update 경계에서 초기화되지 않고 계속 이어지므로 다음 update에서 terminal이 발생한다.

## 5. 조밀 보상이 실제로 들어오는지 확인하는 로그

동일한 첫 update 스모크에서 terminal 경기는 0이었지만 `shaping_reward_nonzero_rate=0.962890625`였다. 즉 학습자 decision의 약 96.3%에서 플레이 중 평가 신호가 0이 아니었다. 평균 절댓값은 `0.0014508`, 관측 최대 절댓값은 `0.0135548`이었다.

성분별 평균 절댓값도 다음 키로 기록한다.

```text
shaping_stack_mean_abs
shaping_holes_mean_abs
shaping_pending_mean_abs
shaping_ready_mean_abs
shaping_combo_mean_abs
shaping_back_to_back_mean_abs
```

`mean_reward`만 보면 양수·음수가 서로 상쇄되어 0에 가까울 수 있다. 이는 보상이 없다는 뜻이 아니다. `shaping_reward_nonzero_rate`, `shaping_reward_mean_abs`와 성분별 값을 같이 봐야 한다.

## 6. 재개 방법

변경 전 컨테이너는 실행 당시 빌드된 binary를 계속 사용하므로 자동으로 빨라지지 않는다. 중지된 상태에서 변경 사항이 commit된 뒤 다음 명령으로 재개한다.

```powershell
./scripts/run-versus-selfplay.ps1 `
  -ResourceProfile max `
  -Hours 24 `
  -OutputDir checkpoints/versus-selfplay-r2 `
  -Resume
```

모델·optimizer·진행 중 경기·상대 배정은 `latest.pt`에서 복구된다. 계산 순서 최적화 때문에 이전 binary와 이후 random trajectory가 bit-for-bit 같다고 보장하지는 않지만, reward와 PPO 목적함수의 의미는 바뀌지 않는다.

## 7. 다음 판단 기준

현재 로그만으로 shaping 가중치를 다시 바꾸지는 않는다. 먼저 여러 update에서 다음을 확인한다.

1. `shaping_reward_nonzero_rate`가 충분히 높고 한 성분만 지배하지 않는가.
2. `learner_terminal_*`가 누적되며 bootstrap/historical score가 개선되는가.
3. 높이·구멍·danger가 악화되지 않으면서 outgoing attack이 증가하는가.

terminal 성과가 개선되지 않는데 특정 shaping 성분만 과도하면 그때 해당 potential weight를 ablation한다. 공격·T-spin 직접 보상은 held-out 승률을 개선한다는 통제 실험 전에는 추가하지 않는다.
