# 1대1 자기대전 강화학습 실행과 보조 보상

## 1. 현재 구현 범위

승격된 솔로 모방학습 모델을 초기 정책으로 사용하는 placement-level 1대1 PPO 학습기가 구현되어 있다. 모델은 키 입력을 고르지 않고 Hold를 포함한 도달 가능한 최종 착지를 고른다. Rust `BattleSession`이 선택을 12 frame/piece, 즉 5 PPS의 공통 cadence로 적용하면서 공격, 동시 상쇄, 가비지 이동·삽입, B2B, 콤보, Surge와 KO를 처리한다.

학습 모델은 총 4,515 parameter다. 기존 `10→64→32→1` 솔로 scorer를 그대로 포함하고 10개 대전 문맥 특징, 반대칭 value head를 추가했다. 새 문맥 branch의 마지막 층은 0으로 초기화되므로 강화학습을 시작하는 순간의 착지 순위는 기존 솔로 모델과 정확히 같다.

상대 구성은 다음과 같다.

- 50%: 현재 정책과 현재 정책의 자기대전
- 30%: 이전 update에서 보존된 과거 모델
- 20%: 강화학습 전의 고정 솔로 초기 정책

과거 모델이 아직 없는 첫 update에서는 해당 몫도 고정 초기 정책을 사용한다. 고정 상대 경기에서는 양쪽 진영을 번갈아 배정하고 학습 중인 쪽의 transition만 PPO update에 사용한다.

## 2. 보조 보상의 정확한 식

실제 목표 보상은 라운드 종료 시의 승패뿐이다.

```text
승리 z = +1
패배 z = -1
무승부 z = 0
진행 중 z = 0
```

승패가 발생하기 전에도 학습 방향을 알려 주기 위해 다음 잠재함수 `Phi`를 사용한다.

```text
d1 = clip((상대 최대 높이 - 내 최대 높이) / 20, -1, 1)
d2 = clip((상대 구멍 수 - 내 구멍 수) / 16, -1, 1)
d3 = clip((상대 대기 가비지 - 내 대기 가비지) / 20, -1, 1)
d4 = clip((상대 즉시 도착 가비지 - 내 즉시 도착 가비지) / 8, -1, 1)
d5 = clip((내 콤보 - 상대 콤보) / 4, -1, 1)
d6 = clip((내 B2B - 상대 B2B) / 8, -1, 1)

Phi(s) = 0.35*d1 + 0.20*d2 + 0.25*d3
       + 0.10*d4 + 0.05*d5 + 0.05*d6
```

각 `d`는 `[-1, 1]`이고 가중치 합은 1이므로 `Phi`도 항상 `[-1, 1]`이다. 플레이어를 맞바꾸면 모든 차이가 반대가 되므로 다음 관계가 성립한다.

```text
Phi(swap(s)) = -Phi(s)
```

한 transition의 최종 보상은 다음과 같다.

```text
gamma = 0.997
lambda = 0.10
F(s, s') = gamma * Phi(s') - Phi(s)
r = z + lambda * F(s, s')
```

종료 상태의 `Phi`는 강제로 0으로 둔다. 따라서 할인된 보조 보상을 한 게임 전체에 합치면 중간 항이 모두 소거된다.

```text
F0 + gamma*F1 + ... = -Phi(s0) + gamma^T*Phi(sT)
```

`Phi(sT)=0`이고 같은 초기 상태에서 출발하므로 남는 값은 정책이 바꿀 수 없는 상수 `-Phi(s0)`뿐이다. 즉 보조 보상은 어느 중간 상태가 승리에 가까운지 빠르게 알려 주지만, 최종적으로 승리 대신 높이·구멍 점수를 수집하는 별도 목표를 만들지 않는다.

예를 들어 현재 `Phi=0.37375`, 다음 `Phi=0.5`인 진행 중 상태라면 보조 보상은 다음과 같다.

```text
0.10 * (0.997*0.5 - 0.37375) = +0.012475
```

반대로 그 transition에서 바로 승리하면 종료 `Phi=0`이므로 보상은 `1 - 0.037375 = 0.962625`다. 마지막 한 수만 보면 1보다 작지만, 앞선 모든 보조 보상을 할인해 합치면 초기 상태가 정한 상수만 남으므로 승리의 가치가 약해지는 것은 아니다.

현재 가중치는 높이와 가비지 압력을 중심으로 학습 분산을 줄이기 위한 v1 값이다. 정책 목표 보존은 가중치의 직감이 아니라 `potential difference`, 경계 제한, terminal 정규화와 player-swap 반대칭성으로 보장한다. 최종 모델 선정 때는 `shaping_scale=0` 대조군과 비교하여 학습 속도·승률·보상 해킹 여부를 확인한다.

## 3. 실행 방법

프로젝트 최상위 PowerShell에서 실행한다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile balanced -Hours 24
```

자원 프로필은 학습 의미를 바꾸지 않고 Torch 및 Rust 후보 생성 thread 수만 바꾼다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile light -Hours 24
./scripts/run-versus-selfplay.ps1 -ResourceProfile balanced -Hours 24
./scripts/run-versus-selfplay.ps1 -ResourceProfile max -Hours 24
```

중단 후 이어서 실행할 때는 다음과 같이 한다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile max -Hours 24 -Resume
```

한 update가 끝날 때마다 다음 파일이 원자적으로 갱신된다.

```text
checkpoints/versus-selfplay-r0/latest.pt   # optimizer와 정확 재개 상태
checkpoints/versus-selfplay-r0/model.pt    # 솔로 파일 없이 로드되는 실사용 모델
checkpoints/versus-selfplay-r0/snapshots/ # 상대 풀과 사후 평가용 과거 모델
```

진행 중인 32경기는 update가 끝나도 초기화하지 않는다. `latest.pt`에는 각 경기의 현재 seed와 지금까지 선택한 양쪽 candidate index가 함께 들어간다. Resume 시 Rust 엔진이 이 선택 이력을 다시 실행하여 보드·bag·공격·가비지·margin frame을 복원한다. 따라서 한 경기가 600수 이후의 가비지 배율 구간까지 이어지면서도 update 경계 체크포인트에서 정확히 재개된다.

개발용 1 update만 확인하려면 `-MaxUpdates 1`을 사용할 수 있지만, 이는 성능 학습이 아니라 배선 검증이다.

## 4. 짝지은 평가

후보와 과거 모델을 같은 seed에서 좌우 진영을 바꾸어 평가한다.

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.versus `
  --candidate checkpoints/versus-selfplay-r0/model.pt `
  --opponent checkpoints/versus-selfplay-r0/snapshots/update-000010-model.pt `
  --output runs/evaluation/versus-r0-vs-update10.json `
  --base-seed 80001 `
  --seeds 256 `
  --horizon 2000 `
  --frames-per-placement 12 `
  --threads 6 `
  --allow-observed
```

학습 loss만으로 최종 모델을 고르지 않는다. 좌우 교대 승률, 미종료 경기 수, 고정 초기 정책 상대 퇴보 여부와 8/12/15 frame cadence 민감도를 함께 확인한 뒤 champion을 정한다.

## 5. 검증된 사항

- player swap 시 `Phi`와 value가 정확히 부호 반전
- 종료 상태 정규화와 할인 보조 보상의 telescoping 단위 테스트
- 1대1 Rust 배치 → Python actor → PPO 역전파 → `latest.pt`/`model.pt` 저장 smoke
- 완료 update 단위 resume와 의미 설정 hash 불일치 차단
- seed/action history 재연을 통한 진행 중 1대1 경기의 정확 상태 복원
- 자기대전·과거 모델·고정 초기 모델 상대 풀 smoke
- 동일 모델의 좌우 교대 closed-loop 평가 smoke
- 16경기 후보 생성 4회 benchmark: Rayon 1 thread 3.496초, 8 thread 0.741초

마지막 benchmark는 해당 개발 장비의 smoke 측정값이며 장기 학습 처리량 보장은 아니다.
