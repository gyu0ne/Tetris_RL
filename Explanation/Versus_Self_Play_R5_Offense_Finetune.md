# 1대1 자기대전 r5: 공격형 미세조정

## 1. 목적

r4 update 870~891 로그에서 모델은 역사 상대 46.5%, bootstrap 상대 58.3%의 승점을 유지했지만 실제 송신 공격은 약 0.145줄/피스, Full T-spin은 0.009회/100피스에 머물렀다. 승패 trace의 평균 절댓값은 0.135였고 기존 potential shaping trace는 0.00287이었다. 모방학습으로 이미 장기 생존이 가능한 정책에는 이 구조가 공격을 새로 발견하기보다 상대가 먼저 무너지기를 기다리는 방향으로 작동할 수 있다.

r5의 목표는 생존 prior를 버리는 것이 아니라, 실제 상대 압력을 만들지 않는 정책이 학습상 손해를 보도록 만드는 것이다. 기술 이름에는 보상하지 않으며 T-spin, Tetris, combo 중 어떤 수단으로든 유효한 순공격을 만들 수 있다.

## 2. 순공격 보상

동시에 선택된 두 착지의 자기 상쇄 이후 실제 송신량을 각각 `g0`, `g1`이라 한다.

```text
relative = clamp((g0 - g1) / 4, -1, 1)
r0 = alpha(update) * relative
r1 = -r0
```

- 양쪽이 같은 공격을 보내면 0이다.
- 상대보다 1줄 더 보내면 초기 `+0.005`, 4줄 이상 더 보내면 최대 `+0.02`다.
- 자기 incoming garbage를 지우는 데 소모된 공격은 `outgoing`에 포함되지 않는다.
- 줄 삭제, Mini, T-spin, Tetris라는 분류 자체에는 보상하지 않는다.
- 매 placement 보상은 `[-0.02, 0.02]`로 제한된다.

따라서 1줄 공격을 네 번 나누거나 한 번에 4줄을 보내는 선형 구간의 총 보상은 같지만, 한 피스에 더 많은 공격을 보내는 편이 시간·피스 효율에서 유리하다. 동일 공격 교환은 0이고 상대 공격을 받으면서 버티기만 하면 음의 보상을 받는다.

## 3. 시간 감쇠와 모방 제약 해제

공격 계수는 처음 150 update 동안 `0.02`이며 update 400까지 `0.005`로 선형 감쇠한다. 최종 승패 `+1/-1`과 r3/r4 potential shaping은 그대로 유지된다. 직접 공격 보상은 potential invariant가 아니므로 0이 아닌 작은 바닥을 남기는 대신 최종 승격을 실제 대국 gate로 제한한다.

모방 정책을 붙잡는 kickstart 계수는 `0.001`에서 시작해 100 update에 0이 된다. 생존 prior 가중치는 초기 모델에 남지만, actor가 휴리스틱의 보수성을 벗어날 수 있다. entropy는 `0.0001 -> 0.00003`, critic 보강과 안정 리그는 r4와 같다.

## 4. 로그

기존 지표와 함께 다음을 기록한다.

- `offense_reward_coefficient`: 현재 공격 계수
- `offense_reward_mean`, `offense_reward_mean_abs`: learner가 받은 순공격 보상
- `offense_reward_nonzero_rate`, `offense_reward_max_abs`: 발생 빈도와 상한
- `offense_trace_mean_abs`, `offense_trace_nonzero_rate`: GAE 구간에서 전파된 공격 신호
- `terminal_offense_trace_cosine`: 공격 신호와 최종 승패 방향의 정렬 정도

실제 smoke에서 공격 1줄 우위는 `0.005`로 제한됐고, 공격 trace `0.00323`이 기존 shaping trace `0.00162`보다 크게 발생했다. 이는 공격 신호가 단순 로그 장식이 아니라 advantage에 들어감을 확인한 것이며 장기 성능 결과는 아니다.

## 5. 실행

```powershell
./scripts/run-versus-offense-finetune.ps1
```

실행기는 다음을 한 번에 수행한다.

1. r4 `selected-model.pt`가 없으면 update 50 간격 후보를 먼저 대국 평가한다.
2. 선택된 r4 모델에서 r5를 새 optimizer·리그로 시작한다.
3. 최대 400 update를 학습한다. 현재 실측 속도에서는 약 7시간이며 `Hours=12`는 안전 상한이다.
4. r5 후보를 r3 anchor와 r4 baseline에 대해 8/12/15 cadence로 평가한다.
5. 모든 공격·승점·안정성 gate를 통과한 후보만 승격한다.

중단 후에는 다음처럼 정확 재개한다. `Hours`는 재실행 시점부터의 새 wall-clock 상한이지만 `MaxUpdates=400`은 누적 update 상한이므로 추가 400 update를 잘못 돌리지 않는다.

```powershell
./scripts/run-versus-offense-finetune.ps1 -Resume
```

## 6. 승격 gate

r4 baseline과 같은 고정 r3 anchor, seed, 좌우 진영, cadence를 사용한다.

- 고정 상대 평균·최악 승점: baseline보다 3%p 넘게 하락 금지
- r4 baseline 직접 대국: 승점 47% 이상
- outgoing attack/piece: baseline의 120% 이상
- danger rate: baseline의 115% 이하
- holes/piece: baseline의 115% 이하

통과 후보가 없으면 r5 최신 모델을 억지로 채택하지 않고 r4 baseline을 `selected-model.pt`로 복사한다. 따라서 공격 보상이 실패해도 실사용 모델의 대국 성능을 자동으로 후퇴시키지 않는다.
