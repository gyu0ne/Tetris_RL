# 1대1 자기대전 r7: 공격 유도형 실험

## 1. r6에서 확인된 실제 병목

r6은 update 100에서 의도대로 자동 중단됐다. 고정 평가의 r4 outgoing attack/piece는 `0.08675`, r6은 `0.087125`로 `1.0043x`에 불과했다. 내부 rollout에서도 공격 후보가 있을 때 약 80%를 선택했고, 4줄 이상 후보는 거의 100% 선택했다. 반면 공격 후보가 존재하는 decision은 약 10%, 4줄 후보는 약 0.5%뿐이었다.

따라서 병목은 공격 수를 보고도 피하는 것이 아니라 공격 가능한 판형을 충분히 만들지 못하는 데 있었다. r7은 생존 정책에 공격 보상을 조금 더 섞는 방식이 아니라, 공격 준비와 실제 공격을 policy의 우선 목표로 둔다.

## 2. 세 advantage 채널

전체 GAE를 다음 세 부분으로 나눈다.

- `A_attack`: 양쪽의 실제 순송신 공격 차이에서 전파된 trace
- `A_setup`: combo, B2B, 다음 수 최대 outgoing, Full T-spin 공격 준비도 변화에서 전파된 trace
- `A_base = A_total - A_attack - A_setup`: 승패, critic correction, 구조적 생존·방어 등 나머지

정책에는 다음 조합을 사용한다.

```text
A_policy = standardize(
    0.25 * standardize(A_base)
  + 2.00 * standardize(A_setup)
  + 3.00 * standardize(A_attack)
)
```

critic은 계속 전체 실제 return을 학습한다. `A_setup`은 기존 potential shaping 안에 이미 포함된 네 component를 분리한 것이므로 환경 보상을 이중으로 더하지 않는다. terminal transition에서는 setup 채널을 0으로 두고, 승패와 terminal에서 사라지는 준비도의 영향은 base 채널에 남긴다.

이 가중치는 공격을 실험하기 위한 의도적인 목적 변경이다. 생존 base가 공격 채널보다 네 배 크게 지배했던 이전 구조와 달리, r7은 공격 준비·실행 합계가 base보다 훨씬 강하다.

## 3. 실제 공격 효용

```text
delta = (g0 - g1) / 4
r0_attack = 0.10 * sign(delta) * clamp(abs(delta), 0, 1)^2
r1_attack = -r0_attack
```

- 1줄 순우위: `0.00625`
- 2줄 순우위: `0.025`
- 3줄 순우위: `0.05625`
- 4줄 이상 순우위: `0.10`

제곱 효용은 단발 1줄보다 3~4줄 spike를 강하게 선호한다. 양쪽 합은 항상 0이며 기술 이름 자체에는 event reward를 주지 않는다.

## 4. 보수적 prior 해제

- solo afterstate scorer learning-rate multiplier: `0.1 -> 1.0`
- actor learning rate: `4e-4`
- tactical potential fraction: `0.25 -> 0.50`
- tactical candidate curriculum: `0.003 -> 0.0005` over 100 updates
- tactical temperature: `0.25`
- normalized entropy: `0.0005 -> 0.0002`
- target KL: `0.006`
- current self-play / historical / bootstrap: `10 / 75 / 15%`

정책 logit은 솔로 scorer와 versus residual의 합이다. r6의 solo trunk `0.1x`는 안전한 모방 판형을 거의 고정했으므로 r7은 두 부분을 같은 기본 학습률로 바꾼다. 상대는 자기 자신보다 고정·역사 모델 비중을 높여 공격량 비교 대상을 안정화한다.

## 5. 실행과 산출물

깨끗한 작업 트리에서 실행한다.

```powershell
./scripts/run-versus-aggressive-r7.ps1 -ResourceProfile max
```

누적 update `25, 50, 75, 100`을 모두 학습하고, 각 단계에서 같은 r3 상대·seed·좌우 진영으로 r4 baseline과 비교한다. r6처럼 중간 공격 gate로 멈추지 않는다. 중단 후 같은 명령을 다시 실행하면 완료 stage를 재사용하고 `latest.pt`에서 이어간다.

주요 산출물은 다음과 같다.

- `aggressive-model.pt`: 네 단계 중 outgoing attack/piece가 가장 높은 공격 전용 연구 모델
- `aggressive-selection.json`: 공격량, r4 대비 비율, score, danger, holes 비교
- `selected-model.pt`: 별도의 대국력·안정성 gate까지 통과한 경우에만 실사용 후보

`aggressive-model.pt`는 안전성 실패 여부와 무관하게 생성된다. 직접 플레이하며 공격 성향을 확인하기 위한 모델이지 자동 실사용 승격본이 아니다.

## 6. 판정 기준

공격 전용 실험의 성공 기준은 최소 한 stage에서 r4 outgoing attack/piece의 `1.20x`를 넘는 것이다. 공격량이 충분히 증가하면 r7은 목적을 달성한 것이며, holes나 danger가 높더라도 다음 단계에서 공격 성향을 유지한 채 안정성을 되돌리는 재학습 대상으로 사용할 수 있다.

실사용 `selected-model.pt` 승격은 별도로 다음을 요구한다.

- 공격량 `1.20x` 이상
- 고정 상대 score 하락 `0.05` 이내
- r4 직접 대국 score `0.45` 이상
- danger ratio `1.50` 이하
- holes ratio `1.35` 이하
- 8/12/15 frame cadence와 좌우 진영 교환 평가

## 7. 로그

- `policy_base_advantage_weight`, `policy_setup_advantage_weight`, `policy_offense_advantage_weight`
- `setup_reward_mean_abs`, `setup_trace_mean_abs`, `setup_advantage_std`
- `base_setup_advantage_cosine`, `setup_offense_advantage_cosine`
- 기존 attack/spike opportunity와 capture 지표

신규 smoke update 2에서는 공격 기회율 `0.133`, outgoing attack/piece `0.156`, setup trace nonzero rate `1.0`이 관찰됐다. 같은 짧은 구간에서 mean holes도 `1.289`로 상승했다. 이는 공격 신호가 실제 정책을 움직인다는 배관 검증이며 장기 성능 증거는 아니다.
