# Placement-Level Imitation Model v1

상태: solo bootstrap foundation 구현 완료

결정일: `2026-08-25`

## 목적과 완료 범위

첫 모델은 raw keyboard policy가 아니라 합법적인 locked afterstate 후보마다 점수를 매기는 작은 scorer다. 이번 단계는 engine→teacher→record→loader→loss→checkpoint의 연결을 검증한다. 1대1 승률이 있는 base policy, dataset aggregation, value head와 self-play RL은 아직 완료 범위가 아니다.

엔진의 선언된 learning-relevant solo/TL mechanics에는 현재 알려진 구현 누락이 없다. 다만 version-pinned 외부 reference state/event corpus가 없어 profile은 `OBSERVED_NOT_FUNCTIONALLY_VERIFIED`다. 이 상태는 `--allow-observed`를 명시한 탐색 smoke에는 사용할 수 있지만 최종 동등성·성능 주장에는 사용할 수 없다.

## Action abstraction

모델 action token은 다음 다섯 값이다.

```text
hold, piece, orientation, x, y
```

`arena`는 current piece와 가능한 hold replacement 각각에 대해 geometric SRS+ locked afterstate를 전부 열거한다. movement path는 reachability 탐색과 디버깅에만 사용하며 모델 입력에는 넣지 않는다. 따라서 모델은 finesse나 key timing에 학습 용량을 쓰지 않는다.

향후 1대1 placement arena는 양쪽에 동일한 `frames_per_placement`를 적용해 attack/garbage clock을 진행한다. 기본 실험값은 12 frame/piece(60 Hz에서 5 PPS)이고 8/12/15 frame을 함께 평가한다. 이 cadence는 로컬 계산 budget이며 TETR.IO 입력 mechanics의 일부라고 주장하지 않는다.

## Teacher와 feature contract

v1 feature 순서는 고정되어 있다.

1. `landing_height_x2`
2. `eroded_piece_cells`
3. `row_transitions`
4. `column_transitions`
5. `buried_holes`
6. `cumulative_wells`
7. `aggregate_height`
8. `bumpiness`
9. `max_height`
10. `lines_cleared`

teacher는 고전 Dellacherie 여섯 feature coefficient를 milli-unit 정수로 옮긴 선형 evaluator다. `landing_height_x2` 때문에 해당 weight만 절반 scale을 사용한다. authoritative engine에는 float를 추가하지 않으며 checked `i64` dot product만 계산한다. 이 teacher는 solo stack bootstrap 기준선이지 TL 공격 전략 teacher가 아니다.

## Record와 보관 정책

각 decision은 observation, seed/match/ply, rules 및 engine provenance, teacher ID/config hash, 모든 candidate의 action/checksum/feature/score/rank, chosen index와 top-two margin을 보존한다. 후보 전체 score를 남기는 이유는 chosen-only classification, soft distillation과 rank loss를 같은 label에서 비교하기 위해서다.

records는 mtime 0, gzip level 6의 deterministic `.jsonl.gz`이며 SHA-256을 manifest에 기록한다. loader는 다음을 학습 전에 거부한다.

- schema/action/feature contract 불일치
- records SHA-256 불일치
- manifest와 record의 rules/engine/status 불일치
- 후보 rank가 `0..N-1` permutation이 아닌 경우
- rank 0과 chosen index가 다른 경우
- opt-in 없는 `OBSERVED` mechanics

대용량 records와 checkpoint는 Git에 commit하지 않는다. records는 학습과 offline 평가 동안만 존재하는 임시 shard다. checkpoint에 전체 manifest, feature mean/std, model config, teacher/rules/engine provenance와 학습 설정을 넣으므로 shard는 평가 후 삭제하고 같은 config/seed로 재생성할 수 있다. 장기적으로는 Rust generator가 Python trainer에 batch를 직접 스트리밍한다.

## Model과 loss

모델은 candidate feature마다 같은 MLP를 적용한다.

```text
10 features -> Linear(64) -> ReLU -> Linear(32) -> ReLU -> Linear(1)
```

총 trainable parameter는 2,817개다. raw grid CNN보다 먼저 이 구조를 쓰는 이유는 compact afterstate feature와 linear/classification policy의 Tetris 연구 계보, 작은 CPU inference budget, variable candidate set의 자연스러운 처리 때문이다. spatial encoder는 같은 wall-time에서 closed-loop strength 우위를 보여야 추가한다.

teacher score `q_j`, scale `c=1000`, temperature `tau`에 대해 target은 다음과 같다.

```text
p_teacher(j|s) = softmax(q_j / (c * tau))
L = -sum_j p_teacher(j|s) log softmax(f_theta(x_j))
```

feature normalization은 train split에서만 계산한다. `seed % 5 == 0` match 전체를 validation으로 두므로 같은 match의 decision이 양쪽 split에 섞이지 않는다.

## Reproducible smoke result

설정은 `configs/training/solo_imitation_smoke_v1.json`에 있다.

- decisions: 512
- candidate range: 68..72
- train/validation: 448/64 decisions
- epochs: 3
- checkpoint parameters: 2,817
- validation top-1: 59.375%
- validation mean teacher regret: 1,961.234375 milli-score
- self-contained checkpoint size: 16,317 bytes

이 수치는 schema, deterministic generation, loader와 gradient update가 연결됐음을 증명한다. 데이터가 작고 동일 solo teacher rollout에서 나왔으므로 architecture 우위, 실제 생존 성능 또는 1대1 strength를 증명하지 않는다.

## 다음 gate

1. fixed-cadence placement-level `BattleSession` adapter
2. attack/cancel/incoming/B2B/Surge/opponent danger feature
3. 공격·downstack·B2B·안전 style teacher ensemble
4. 100k-decision 생성/학습/삭제 benchmark
5. held-out closed-loop match에서 linear teacher, chosen-only BC와 full-score distillation 비교
6. learner-state dataset aggregation 뒤에만 RL 초기 checkpoint 승격
