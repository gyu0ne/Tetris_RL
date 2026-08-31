# Versus Offense Fine-tune r5

## Settled decision

r4의 potential-only 목적은 생존 prior가 매우 강한 초기 모델에서 공격 발견을 충분히 유도하지 못했다. r5는 실제 자기 상쇄 후 송신된 두 선수의 `outgoing attack` 차이에만 bounded zero-sum event reward를 부여한다. 기술 분류나 gross clear attack은 보상하지 않는다.

```text
reward_pair = +/- alpha(update) * clamp((outgoing_0 - outgoing_1) / 4, -1, 1)
alpha: 0.02 through update 150, linearly to 0.005 at update 400
```

직접 event reward는 potential invariance를 갖지 않는다. 이 목적 변경은 공격하지 않는 생존 정책이 프로젝트 목표에 부합하지 않는다는 사용자 결정에 따른다. 위험은 반대칭, 실제 순송신량, 4줄 정규화, `0.02` 상한, 계수 감쇠와 held-out promotion gate로 제한한다.

## Training boundary

- initialization: cadence-robust r4 selected checkpoint
- duration: maximum 400 updates, not another 24-hour from-scratch run
- kickstart: `0.001 -> 0` by update 100
- stable league and critic: retain r4 v5 mechanisms
- progress schema: `versus-selfplay-ppo-progress-v5`
- config schema: `versus-selfplay-ppo-v6`

## Promotion boundary

Candidate promotion requires fixed-anchor score and robust score within -0.03 of r4, direct r4 score at least 0.47, outgoing attack ratio at least 1.20, and danger/holes ratios at most 1.15. Failure of every candidate retains the r4 baseline. Selection remains paired-side and cadence-sensitive.

## Verification contract

- exact zero-sum/cap/schedule unit tests
- v1-v5 config/checkpoint compatibility tests
- fresh and resumed v6 native environment smoke with nonzero offense reward
- baseline-retention selector smoke
- full Python/Rust/container gates
