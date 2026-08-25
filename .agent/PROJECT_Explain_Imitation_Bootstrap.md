# 휴리스틱 기록 기반 모방학습 Bootstrap

상태: solo 장기학습·평가·learner-state aggregation 실행 파이프라인 구현 완료, 실제 장기 run과 1대1 teacher/RL은 미실행

결정일: `2026-08-24`

구현 갱신일: `2026-08-25`

## 구현된 첫 단계

- `arena` crate가 hold 분기를 포함한 모든 geometric locked afterstate를 열거한다.
- policy action은 `hold + piece + orientation + x/y`이며 movement path는 모델 입력이 아닌 진단용 길이만 남긴다.
- 10개 정수 feature와 milli-scaled Dellacherie 계열 linear teacher를 사용한다.
- 모든 후보의 action, board checksum, feature, teacher score/rank와 immediate clear event를 deterministic gzip JSONL에 저장한다.
- manifest와 record의 SHA-256/rules/engine/status/action schema를 Python loader가 검증하며 `OBSERVED` 데이터는 명시적 opt-in 없이는 거부한다.
- CPU PyTorch 2.8.0의 `10→64→32→1` 공유 scorer가 후보 score soft target을 listwise distill한다.
- checkpoint는 dataset manifest, feature mean/std, model/training config, best epoch와 전체 history를 포함한다. 승격 checkpoint에는 후보 선택·offline·closed-loop 보고서도 포함한다. 대용량 shard는 임시 산출물이며 최종 checkpoint 재로드 후 삭제할 수 있다.
- 매 게임 seed는 `base_seed + seed_stride × match_index`이며 같은 dataset 안에서 중복되지 않는다.
- 장기 trainer는 bounded-memory deterministic shuffle, minimum epoch, patience early stopping과 validation regret best-epoch 복원을 사용한다.
- 세 초기화 seed의 후보를 실제 Rust closed loop에서 고르고, top-out이 있으면 learner가 방문한 상태 250,000개에 같은 Rust teacher label을 붙여 scratch 재학습한다.

512-decision smoke는 448 train/64 held-out decision으로 분리됐고 3 epoch 뒤 held-out top-1 59.375%, mean teacher regret 1,961.234375 milli-score를 기록했다. 이는 end-to-end 연결과 학습 감소를 확인한 결과일 뿐, 1대1 strength나 architecture 채택 근거가 아니다.

## 결정

강화학습 전에 heuristic/search teacher가 생성한 대량의 state-candidate-action 기록으로 기본 모델을 pretrain한다. 다만 one-shot behavior cloning을 최종 정책으로 사용하지 않고, 다음 순서를 채택한다.

```text
검증된 mechanics
  -> 여러 heuristic/search teacher와 상대 pool
  -> 후보 전체 점수 포함 trajectory dataset
  -> behavior cloning + value/rank distillation
  -> learner rollout 상태에 teacher를 다시 질의하는 dataset aggregation
  -> terminal 승패 목적의 self-play RL fine-tuning
```

## 근거

- 2010년 Tetris imitation 연구는 placement 선택을 classification으로 변환해 서로 다른 인간 style을 모방할 수 있음을 보였다. 다만 오래된 별도 환경이므로 TL 성능 근거는 아니다.
- DAgger는 learner의 행동이 이후 state distribution을 바꿔 독립적인 demonstration만으로 학습한 정책의 오류가 누적되는 문제를 다룬다.
- DQfD는 supervised demonstration loss와 temporal-difference 학습을 함께 사용해 초기 학습을 가속할 수 있음을 보였다. variable afterstate action을 쓰는 이 프로젝트에 알고리즘을 그대로 복사하지 않고 원리만 적용한다.
- Tetris CBMPI 연구는 강한 기존 policy가 방문한 state와 rollout 비교를 이용한 classification/policy improvement가 sample-efficient한 기준선이 될 수 있음을 보였다.
- 여러 수준의 teacher를 섞으면 약한 행동까지 동등하게 모방할 위험이 있으므로 teacher identity, strength와 state별 margin을 보존한다.

## 데이터 원칙

- 선택 행동 하나만 저장하지 않고 모든 legal afterstate token, teacher score, rank와 top-2 margin을 저장한다.
- `rules_hash`, engine revision, seed, player side, opponent, teacher weight/search budget hash, observation/action schema version을 필수 provenance로 둔다.
- split은 row 단위가 아니라 seed와 match 단위로 분리한다.
- deterministic tie 하나만 반복하지 않도록 여러 teacher weight/style, node budget과 tie seed를 사용한다.
- solo heuristic 기록은 board feature bootstrap에만 사용한다. 최종 base policy는 attack/garbage/상대 상태가 있는 1 대 1 teacher 기록으로 다시 학습한다.
- dataset/checkpoint 본체는 저장소에 commit하지 않고 manifest, schema, 생성 config와 검증 report만 commit한다.

## 손실과 단계

1. teacher top action에 대한 cross-entropy 기준선을 만든다.
2. near-tie에서 과도하게 확신하지 않도록 candidate score를 temperature soft target 또는 pairwise/listwise rank loss로 distill한다.
3. teacher rollout의 terminal outcome/search value로 value head를 pretrain하되 reward shaping label을 섞지 않는다.
4. learner를 local engine에서 rollout하고 learner가 방문한 state에 teacher label을 추가한다.
5. dataset aggregation 뒤 closed-loop strength가 개선될 때만 RL 초기 checkpoint로 승격한다.
6. RL 단계에서는 imitation loss를 별도 ablation하고 점차 줄이거나 demonstration replay 비율을 제어한다.

## 데이터 규모 게이트

최종 solo bootstrap의 현재 고정 예산은 다음과 같다.

- 기본 교사 dataset: 최소 `1,000,000` decisions, 4,096개 이상의 서로 다른 game seed
- 독립 학습: initialization seed 3개, 최대 100 epoch, 최소 20 epoch, patience 10
- 분포 이탈 보정: learner-state `250,000` decisions × 최대 2회
- 후보 선택: 256 seed × 2,000 placement에서 top-out 0
- 최종 승격: 별도 2,000 seed × 10,000 placement에서 top-out 0

이 값은 짧은 pipeline smoke가 아니라 하루 단위 장기 실행 예산이다. 최종 run 결과가 생기기 전에는 성능 완료로 기록하지 않는다.

## 평가

offline top-1 accuracy만으로 채택하지 않는다.

- held-out seed/match의 top-1, top-k, KL/rank correlation
- teacher action과 다른 경우의 teacher score regret
- closed-loop teacher·scripted·held-out style 상대 승률
- learner rollout에서의 illegal/unreachable action 0건
- inference latency, dataset bytes/decision, generation decisions/s
- BC only, BC+aggregation, terminal-only RL, BC→RL의 paired comparison

## 실패 방지

- **teacher ceiling:** RL과 search improvement가 teacher를 넘어설 경로를 유지한다.
- **covariate shift:** learner-state aggregation을 필수로 둔다.
- **약한 teacher 오염:** teacher strength/margin 기반 sampling과 ablation을 사용한다.
- **style collapse:** 공격·downstack·B2B 유지·안전형 teacher를 분리하고 style별 평가한다.
- **잘못된 mechanics 대량 증폭:** target conformance gate 전에는 정식 dataset을 만들지 않는다.
