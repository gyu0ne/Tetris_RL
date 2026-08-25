# 솔로 모방학습 실행 절차

작성일: `2026-08-25`

대상 설정: `configs/training/solo_imitation_bootstrap_v1.json`

## 이번 실행에서 만드는 것

휴리스틱 교사가 실제 엔진에서 선택한 착지 후보를 약 10만 번 생성하고, 이를 이용해 `afterstate-scorer-v1` 모델을 5 epoch 학습한다. 결과물은 다음 체크포인트 하나다.

```text
checkpoints/solo-imitation-bootstrap-v1/model.pt
```

이 모델은 안전한 solo stack을 배우는 초기 모델이다. 공격, garbage 대응, B2B/Surge 운영과 상대 관찰은 아직 학습하지 않으므로 1대1 완성 모델이 아니다.

## 고정 설정

```text
교사: dellacherie-linear-v1
seed 시작: 10001
게임 수: 512
게임당 최대 결정: 200
목표 결정 수: 102,400
허용 최소 결정 수: 100,000
모델: 10 -> 64 -> 32 -> 1
batch: 64 decisions
epoch: 5
learning rate: 0.0003
teacher temperature: 1.0
teacher score scale: 1000
CPU thread: 2
학습 seed: 2026
```

512개 게임을 각각 200수까지 진행한다. 교사가 중간에 top-out하면 해당 게임은 그 시점에서 끝나므로 실제 결정 수가 102,400보다 작을 수 있다. manifest의 실제 결정 수가 100,000 미만이면 `--matches 600`으로 전체 shard를 다시 만든다.

## 0단계: 실행 전 조건

데이터에 기록되는 `engine_revision`은 실제 소스와 일치해야 한다. 따라서 본학습 전 작업 트리가 clean인 커밋 상태여야 한다.

```powershell
git status --short
git rev-parse HEAD
```

`git status --short`에 출력이 있으면 먼저 현재 소스와 문서를 검증하고 commit한다. 임의의 revision 문자열이나 변경 전 commit hash로 본학습 records를 만들지 않는다.

컨테이너가 준비됐는지 확인한다.

```powershell
docker compose build rust
docker compose build training
docker compose run --rm rust cargo test --workspace --all-targets
docker compose run --rm training python -m unittest discover -s python/tests -v
```

## 1단계: 10,240-decision 속도 측정

본학습 전에 같은 형식으로 작은 benchmark를 한 번 실행한다. 이 단계는 학습량을 결정하는 용도가 아니라 현재 PC에서 생성 시간과 임시 저장 공간을 확인하는 용도다.

```powershell
$revision = (git rev-parse HEAD).Trim()

Measure-Command {
  docker compose run --rm rust cargo run --release -p arena --bin generate-solo -- --records datasets/solo-imitation-benchmark-v1/records.jsonl.gz --manifest datasets/solo-imitation-benchmark-v1/manifest.json --engine-revision $revision --seed 9001 --matches 64 --decisions-per-match 160
}
```

결과를 확인한다.

```powershell
$benchmark = Get-Content -LiteralPath 'datasets/solo-imitation-benchmark-v1/manifest.json' -Raw | ConvertFrom-Json
$benchmark | Select-Object decisions, min_candidates, max_candidates
Get-Item -LiteralPath 'datasets/solo-imitation-benchmark-v1/records.jsonl.gz' | Select-Object Length
```

benchmark가 정상이라면 이 폴더는 삭제해도 된다. 본학습은 별도 seed를 사용하므로 benchmark records를 합치지 않는다.

## 2단계: 본학습 records 생성

```powershell
$revision = (git rev-parse HEAD).Trim()

docker compose run --rm rust cargo run --release -p arena --bin generate-solo -- --records datasets/solo-imitation-bootstrap-v1/records.jsonl.gz --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --engine-revision $revision --seed 10001 --matches 512 --decisions-per-match 200
```

생성 후 반드시 실제 개수를 확인한다.

```powershell
$manifest = Get-Content -LiteralPath 'datasets/solo-imitation-bootstrap-v1/manifest.json' -Raw | ConvertFrom-Json
$manifest | Select-Object decisions, completed_matches, min_candidates, max_candidates, engine_revision, mechanics_status
```

판정은 다음과 같다.

- `decisions >= 100000`: 그대로 학습한다.
- `decisions < 100000`: 같은 출력 경로에 `--matches 600`으로 다시 생성한다.
- `engine_revision`이 현재 `git rev-parse HEAD`와 다름: shard를 폐기하고 다시 생성한다.
- `mechanics_status`가 `OBSERVED_NOT_FUNCTIONALLY_VERIFIED`가 아님: 원인을 확인하기 전 학습하지 않는다.

## 3단계: 5-epoch 학습

```powershell
docker compose run --rm training python -m tetris_rl.training.imitation --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --output checkpoints/solo-imitation-bootstrap-v1/model.pt --epochs 5 --batch-decisions 64 --learning-rate 0.0003 --teacher-temperature 1.0 --teacher-score-scale 1000 --seed 2026 --threads 2 --allow-observed
```

각 epoch마다 JSON 한 줄이 출력된다. 현재 확인할 값은 다음 두 개다.

```text
validation.mean_teacher_regret
validation.top_one_accuracy
```

우선순위는 `mean_teacher_regret`가 더 높다. 현재 teacher는 동점 후보가 많으므로 top-one이 달라도 teacher score가 같을 수 있다.

정상 학습의 최소 조건은 다음과 같다.

- train loss가 첫 epoch보다 마지막 epoch에서 낮다.
- validation loss가 발산하거나 `NaN`이 아니다.
- validation mean teacher regret가 첫 epoch보다 마지막 epoch에서 낮다.
- checkpoint 생성 로그의 parameters가 `2817`이다.

현재 trainer는 마지막 epoch만 저장한다. 첫 본학습은 고정 5 epoch로 실행하고, epoch 3 이후 validation regret가 계속 악화하면 checkpoint를 승격하지 말고 best-checkpoint 저장 기능부터 추가한다.

## 4단계: 체크포인트 단독 로드 검증

```powershell
docker compose run --rm training python -c "from pathlib import Path; from tetris_rl.models.checkpoint import load_scorer; loaded=load_scorer(Path('checkpoints/solo-imitation-bootstrap-v1/model.pt'), allow_observed=True); print({'parameters': loaded.model.parameter_count(), 'engine_revision': loaded.metadata['engine_revision'], 'dataset_id': loaded.metadata['dataset_id']})"
```

다음을 모두 만족해야 한다.

- 오류 없이 로드됨
- parameters가 `2817`
- engine revision이 본학습에 사용한 commit과 같음
- dataset ID가 manifest의 dataset ID와 같음

이 검증이 끝나면 체크포인트는 records 없이도 inference에 필요한 feature 순서, 정규화 값, 모델 구조와 provenance를 갖는다.

## 5단계: 기본 성능 판정

5 epoch가 끝났다는 이유만으로 “기본 성능 확보”라고 표시하지 않는다. 구현된 두 평가기를 순서대로 실행한다.

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.offline --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --checkpoint checkpoints/solo-imitation-bootstrap-v1/model.pt --split validation --batch-decisions 64 --threads 2 --allow-observed --require-gates --output runs/evaluation/solo-imitation-bootstrap-v1-offline.json

docker compose run --rm training python -m tetris_rl.evaluation.closed_loop --checkpoint checkpoints/solo-imitation-bootstrap-v1/model.pt --base-seed 20001 --seeds 500 --horizon 1000 --threads 2 --allow-observed --require-gates --min-survival 0.95 --output runs/evaluation/solo-imitation-bootstrap-v1-closed-loop.json
```

평가기는 다음 값을 출력한다.

```text
tie_aware_optimal_rate
positive_margin_agreement
mean_normalized_regret
survival_at_1000
```

첫 승격 기준은 다음과 같다.

```text
tie_aware_optimal_rate >= 0.97
positive_margin_agreement >= 0.95
mean_normalized_regret <= 0.05
survival_at_1000 >= 0.95 on 500 unseen seeds
```

이 기준을 통과하면 보고서를 검증해 최종 솔로 초기 checkpoint로 승격한다.

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.promote --checkpoint checkpoints/solo-imitation-bootstrap-v1/model.pt --offline-report runs/evaluation/solo-imitation-bootstrap-v1-offline.json --closed-loop-report runs/evaluation/solo-imitation-bootstrap-v1-closed-loop.json --output checkpoints/solo-imitation-v1/model.pt --allow-observed
```

tie-aware offline evaluator는 validation records를 읽으므로 이 단계가 끝날 때까지 dataset을 삭제하지 않는다.

## 6단계: records 삭제

승격 체크포인트의 단독 로드 검증이 끝난 뒤 본학습 records와 manifest를 삭제한다. 정확한 대상이 workspace 아래인지 확인한 다음 삭제한다.

```powershell
$workspace = (Resolve-Path -LiteralPath '.').Path
$target = (Resolve-Path -LiteralPath 'datasets/solo-imitation-bootstrap-v1').Path
$prefix = $workspace.TrimEnd('\') + '\'

if (-not $target.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "Target outside workspace: $target"
}

Remove-Item -LiteralPath $target -Recurse -Force
```

부트스트랩 체크포인트와 외부 평가 JSON도 최종 승격 checkpoint에 평가 metadata가 포함된 것을 확인한 뒤 삭제할 수 있다. 남기는 파일은 다음 하나다.

```text
checkpoints/solo-imitation-v1/model.pt
```

## 실패했을 때의 구체적인 대응

### validation regret도 높음

같은 10만 records로 epoch를 8까지 늘린다. 8 epoch에서도 좋아지지 않으면 데이터를 늘리지 말고 learning rate `0.0001`, teacher temperature `1.0`을 비교한다.

### offline 평가는 좋지만 closed-loop에서 무너짐

교사 게임을 더 생성하지 않는다. 현재 모델로 게임을 진행해 모델이 방문한 상태 50,000개를 모으고, 각 상태의 모든 후보를 기존 교사로 다시 평가한다. 이를 기존 records와 합쳐 3 epoch 재학습한다. 최대 두 번 반복한다.

```text
초기 teacher states: 100,000
aggregation round 1: learner states 50,000
aggregation round 2: learner states 50,000
최대 총량: 200,000 decisions
```

learner-state generator와 shard merge는 아직 구현되지 않았다. closed-loop 실패가 확인되기 전에 먼저 만들 필요는 없다.

### solo 평가는 통과했지만 1대1에서 약함

정상이다. solo records를 더 늘리지 않는다. 다음 단계에서 attack, garbage, B2B/Surge, 상대 stack feature를 추가하고 versus teacher 또는 self-play RL로 이동한다.

## 최종 작업 순서 요약

```text
현재 변경 commit
  -> 10,240개 생성 속도 측정
  -> 512 games x 200 decisions 생성
  -> 실제 decisions >= 100,000 확인
  -> 5 epoch 학습
  -> checkpoint 단독 로드 검증
  -> tie-aware + 실제 model.pt closed-loop 평가
  -> 통과: 평가 보고서를 내장한 최종 checkpoint 승격
  -> 최종 checkpoint 단독 로드 검증
  -> records 삭제 후 1대1 단계
  -> 실패: 원인에 따라 8 epoch 또는 learner-state 50,000개 집계
```
