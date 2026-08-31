# 1대1 자기대전 r6: 공격 발견 학습

## 1. 왜 r5를 그대로 연장하지 않는가

r5는 실제 순송신 공격을 보상에 넣었지만 update 230까지 공격량이 증가하지 않았다. 초기 50 update의 outgoing attack/piece 평균은 약 `0.1389`, update 181~230은 약 `0.1360`이었다. 최근 PPO의 KL은 약 `0.00036`, clip fraction은 약 `0.0035`라 정책 변화도 매우 작았다. 공격 trace 평균 절댓값 `0.017`은 terminal trace `0.127`보다 훨씬 작았다.

따라서 r6는 같은 보상 계수만 키우지 않는다. 승패·생존 신호와 공격 신호를 정책 advantage에서 분리하고, 실제 공격 가능한 수를 골랐는지 직접 측정한다. 장기 실행 전에 100 update 공격 gate를 두어 실패한 방향에 하루를 더 쓰지 않는다.

## 2. 공격 보상

한 동시 placement에서 두 선수의 자기 pending garbage 상쇄 후 송신량을 `g0`, `g1`이라 하고, 서로 맞부딪힌 공격까지 뺀 순우위를 다음처럼 평가한다.

```text
delta = (g0 - g1) / 4
r0_attack = 0.06 * sign(delta) * clamp(abs(delta), 0, 1)^1.5
r1_attack = -r0_attack
```

- 1줄 대 0줄은 `+0.0075`, 4줄 이상 대 0줄은 `+0.06`이다.
- 양쪽 보상의 합은 항상 0이고 placement당 절댓값은 `0.06` 이하이다.
- 줄 삭제, Tetris, T-spin Mini 같은 기술 이름 자체에는 보상하지 않는다.
- `g`는 자기 incoming garbage를 지우고 남은 outgoing이며, 양쪽의 차이에 효용을 적용하므로 같은 프레임에 맞상대한 공격도 순압력으로 위장할 수 없다.
- 지수 `1.5`는 약한 1줄 공격을 반복하는 것보다 4줄급 spike를 더 높은 피스 효율로 평가한다.

이 보상은 potential shaping이 아니라 공격을 학습 목표에 명시적으로 포함한 유한 기간 curriculum이다. 최종 채택은 별도의 승점·안정성 gate를 통과해야 한다.

## 3. 분리 policy advantage

기존에는 공격 보상이 전체 GAE에 섞여 terminal trace와 critic 오차에 묻혔다. r6는 policy update에 다음 값을 쓴다.

```text
A_base = A_total - A_attack
A_policy = standardize(
    standardize(A_base) + 0.7 * standardize(A_attack)
)
```

critic의 return target은 여전히 전체 실제 보상을 사용한다. actor만 공격 방향을 독립 채널로 받으므로 공격 발생 빈도가 낮아도 신호의 방향이 사라지지 않는다. 공격 trace가 모두 0인 batch에서는 정규화 결과도 0이므로 가짜 gradient는 생기지 않는다.

보조 전술 curriculum은 고정 교사 정책의 후보 순위에 실제 outgoing, garbage cancellation, Full T-spin 공격 준비도를 합쳐 공격 후보를 잠시 더 자주 선택하게 한다. 계수는 `0.0003 -> 0.00005`로 update 150까지 감소한다. 이는 실제 event reward를 대체하지 않고 희귀 공격 상태를 더 자주 경험하게 하는 탐색 보조다.

## 4. 최적화 안정성

- actor learning rate: `3e-4`
- solo trunk learning-rate multiplier: `0.1`
- normalized entropy coefficient: `0.0003 -> 0.0001` over 200 updates
- target KL: `0.003`; 한 minibatch가 `0.0045`를 넘으면 해당 PPO epoch를 조기 종료
- 상대 비율: current self-play 20%, 안정 historical pool 65%, bootstrap 15%
- 시작 모델: 실패한 r5가 아니라 검증된 r4 `selected-model.pt`
- 최대 길이: 200 update

r4의 안정 상대 풀, 시간 감쇠 전적, critic 보강, paired-side 평가 구조는 유지한다.

## 5. 새 로그 해석

- `attack_opportunity_rate`: 현재 legal candidates 중 outgoing 1줄 이상이 존재한 decision 비율
- `attack_capture_rate`: 공격 가능한 decision에서 실제로 outgoing을 만든 비율
- `best_attack_capture_rate`: 가능한 최대 outgoing을 선택한 비율
- `available_attack_capture_ratio`: 선택한 outgoing 합 / 각 decision의 가능한 최대 outgoing 합
- `spike_opportunity_rate`: outgoing 4줄 이상 후보가 있던 비율
- `spike_capture_rate`: 그중 실제 4줄 이상을 선택한 비율
- `base_advantage_std`, `offense_advantage_std`: 두 advantage 채널의 실제 변동 크기
- `base_offense_advantage_cosine`: 생존·승패 방향과 공격 방향의 정렬 정도
- `ppo_early_stopped`, `ppo_epochs_completed`, `approximate_kl`: 업데이트가 과격해 KL 안전장치가 작동했는지 확인

공격량이 낮을 때 `attack_opportunity_rate`도 낮으면 공격 상태를 만들지 못하는 문제이고, 기회는 충분하지만 `attack_capture_rate`가 낮으면 후보 선택 문제다. 두 경우를 더 이상 같은 원인으로 취급하지 않는다.

## 6. 실행

먼저 변경 사항이 모두 commit된 깨끗한 작업 트리에서 실행한다.

```powershell
./scripts/run-versus-offense-r6.ps1 -ResourceProfile max
```

실행기는 누적 update `50, 100, 150, 200`마다 학습을 멈추고 같은 r3 anchor·seed로 r4 baseline과 공격량을 비교한다. 중단 후 같은 명령을 다시 실행하면 존재하는 snapshot과 `latest.pt`를 재사용해 다음 stage부터 정확히 이어간다.

자원 프로필은 다음처럼 바꿀 수 있다.

```powershell
./scripts/run-versus-offense-r6.ps1 -ResourceProfile light
./scripts/run-versus-offense-r6.ps1 -ResourceProfile balanced
./scripts/run-versus-offense-r6.ps1 -ResourceProfile max
```

각 stage의 기본 wall-clock 상한은 4시간이다. 속도가 느린 환경에서는 `-HoursPerStage 8`처럼 늘릴 수 있으며 누적 update 상한은 변하지 않는다.

## 7. 조기 중단과 최종 승격

update 100에서 같은 조건의 r4보다 outgoing attack/piece가 10% 이상 늘지 않으면 `probes/early-stop.json`을 남기고 자동 중단한다. 이 gate는 최종 품질 판정이 아니라 실패한 공격 탐색을 빨리 끊는 장치다.

update 200까지 도달하면 다음 조건을 모두 만족한 후보만 `selected-model.pt`로 승격한다.

- outgoing attack/piece: r4의 `1.20x` 이상
- 고정 r3 anchor 승점: r4보다 `0.03` 초과 하락 금지
- r4 직접 대국 score: `0.47` 이상
- danger rate: r4의 `1.15x` 이하
- mean holes: r4의 `1.15x` 이하
- 좌우 진영 교환 및 8/12/15 frame cadence 평가

공격을 늘리지 못하거나 공격만 늘고 대국력·안정성을 잃은 모델은 채택하지 않는다.
