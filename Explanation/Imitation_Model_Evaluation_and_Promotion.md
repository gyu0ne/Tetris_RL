# 모방학습 모델 평가와 실사용 체크포인트 생성

작성일: `2026-08-25`

이 문서는 휴리스틱 데이터 생성부터 실제 사용할 솔로 초기 모델 `model.pt`를 확정하는 전체 절차를 설명한다. 평가기는 휴대용 JSON 가중치를 사용하지 않는다. PyTorch 체크포인트를 Python에서 그대로 로드해 추론하고, Rust는 권위 엔진의 후보 생성과 상태 전이만 담당한다.

## 두 평가기의 역할

### Offline 평가

학습에 사용하지 않은 validation 결정마다 다음을 수행한다.

1. 당시 가능한 모든 착지 후보를 `model.pt`로 채점한다.
2. 모델이 가장 높게 평가한 후보의 휴리스틱 교사 점수를 확인한다.
3. 교사 최댓값과 같으면, 교사가 고른 인덱스와 달라도 정답으로 인정한다.

따라서 `tie_aware_optimal_rate`는 동점 정답을 올바르게 처리한다. `positive_margin_agreement`는 교사의 최선 후보가 하나뿐인 결정만 따로 측정한다. `mean_normalized_regret`는 모델 선택으로 잃은 평균 교사 점수를 교사의 평균 양수 선택 여유로 나눈 값이다.

이 평가는 빠르고 재현 가능하지만 모델이 자기 실수로 만든 낯선 보드까지는 검사하지 못한다.

### Closed-loop solo 평가

학습·validation에 쓰지 않은 새 seed에서 다음 과정을 반복한다.

```text
Rust 권위 엔진이 모든 합법 착지 후보 생성
  -> Python이 실제 model.pt로 후보 일괄 채점
  -> 최고 점수 후보의 인덱스 반환
  -> Rust 권위 엔진이 그 수를 적용
  -> 다음 상태에서 반복
```

모델이 직접 만든 상태가 다음 입력이 되므로 작은 선택 오류가 누적되는지를 검사할 수 있다. 기본 gate는 새 seed 500개 중 95% 이상이 1,000수까지 생존하는 것이다.

## 왜 JSON 가중치를 쓰지 않는가

JSON은 평가 결과 보고서에만 사용한다. 모델 가중치를 JSON으로 다시 내보내면 PyTorch 체크포인트와 별도 직렬화 형식이 생기고, 두 구현의 수치 일치와 버전 관리를 추가로 증명해야 한다. 현재 구조는 이를 피한다.

- 모델 로드·정규화·추론: Python/PyTorch, 원본 `model.pt`
- 후보 생성·상태 전이: Rust `arena::SoloBatch`
- 경계: PyO3 `tetris_engine.SoloBatch`
- 전달 데이터: 후보 feature의 little-endian `i32` byte buffer, 게임별 offset, 선택 인덱스

모델을 Rust로 변환하거나 복제하지 않으므로 평가와 이후 Python 학습 코드가 같은 가중치를 사용한다.

## 처음부터 실사용 모델을 만드는 절차

### 1. 컨테이너 준비와 깨끗한 revision 확보

```powershell
docker compose build rust
docker compose build training
git status --short
git rev-parse HEAD
```

본 데이터의 `engine_revision`은 실제 생성 코드를 가리켜야 하므로 변경 사항을 검증하고 commit한 뒤 시작한다.

### 2. 휴리스틱 기록 10만 개 생성

```powershell
$revision = (git rev-parse HEAD).Trim()

docker compose run --rm rust cargo run --release -p arena --bin generate-solo -- --records datasets/solo-imitation-bootstrap-v1/records.jsonl.gz --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --engine-revision $revision --seed 10001 --matches 512 --decisions-per-match 200
```

manifest의 `decisions`가 100,000 미만이면 같은 출력 경로에 `--matches 600`으로 다시 만든다. 이 데이터는 학습과 offline 평가가 끝날 때까지만 필요하다.

### 3. 실제 PyTorch 체크포인트 학습

```powershell
docker compose run --rm training python -m tetris_rl.training.imitation --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --output checkpoints/solo-imitation-bootstrap-v1/model.pt --epochs 5 --batch-decisions 64 --learning-rate 0.0003 --teacher-temperature 1.0 --teacher-score-scale 1000 --seed 2026 --threads 2 --allow-observed
```

생성되는 파일은 약 2,817개 parameter의 실제 PyTorch `model.pt`다. 데이터 자체가 아니라 이 체크포인트가 모델이다.

### 4. Offline gate 실행

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.offline --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --checkpoint checkpoints/solo-imitation-bootstrap-v1/model.pt --split validation --batch-decisions 64 --threads 2 --allow-observed --require-gates --output runs/evaluation/solo-imitation-bootstrap-v1-offline.json
```

기본 통과 기준은 다음과 같다.

```text
tie_aware_optimal_rate >= 0.97
positive_margin_agreement >= 0.95
mean_normalized_regret <= 0.05
```

gate 미달이면 프로세스가 종료 코드 `3`으로 끝난다. 이때 체크포인트를 승격하지 않는다.

### 5. Closed-loop gate 실행

먼저 실행 경로만 빠르게 확인하려면 별도 결과 파일 없이 작은 smoke를 실행한다.

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.closed_loop --checkpoint checkpoints/solo-imitation-bootstrap-v1/model.pt --base-seed 20001 --seeds 8 --horizon 100 --threads 2 --allow-observed
```

smoke 통과 후 고정된 본평가를 실행한다.

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.closed_loop --checkpoint checkpoints/solo-imitation-bootstrap-v1/model.pt --base-seed 20001 --seeds 500 --horizon 1000 --threads 2 --allow-observed --require-gates --min-survival 0.95 --output runs/evaluation/solo-imitation-bootstrap-v1-closed-loop.json
```

여기서도 Python이 실제 `model.pt`를 사용한다. Rust 엔진은 500개 게임을 배치로 유지하며 매 수의 합법 후보와 다음 상태를 계산한다.

### 6. 통과 모델 승격

두 보고서가 같은 체크포인트 SHA-256, dataset ID, engine revision을 가리키고 모든 gate를 통과해야 승격할 수 있다.

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.promote --checkpoint checkpoints/solo-imitation-bootstrap-v1/model.pt --offline-report runs/evaluation/solo-imitation-bootstrap-v1-offline.json --closed-loop-report runs/evaluation/solo-imitation-bootstrap-v1-closed-loop.json --output checkpoints/solo-imitation-v1/model.pt --allow-observed
```

승격기는 두 평가 보고서를 체크포인트 내부 `promotion` metadata에 포함하고 다시 로드해 검증한다. 최종적으로 보존할 실사용 솔로 초기 모델은 다음 하나다.

```text
checkpoints/solo-imitation-v1/model.pt
```

### 7. 최종 단독 검증과 임시 파일 정리

```powershell
docker compose run --rm training python -c "from pathlib import Path; from tetris_rl.models.checkpoint import load_scorer; loaded=load_scorer(Path('checkpoints/solo-imitation-v1/model.pt'), allow_observed=True); print({'parameters': loaded.model.parameter_count(), 'engine_revision': loaded.metadata['engine_revision'], 'dataset_id': loaded.metadata['dataset_id'], 'promotion': loaded.metadata['promotion']['schema_version']})"
```

이 명령이 성공한 뒤에는 `datasets/solo-imitation-bootstrap-v1`, 부트스트랩 체크포인트, 외부 평가 JSON을 삭제해도 된다. 평가 결과가 최종 `model.pt` 안에 포함되어 있기 때문이다. 단, 재현이 필요하면 설정·seed·engine revision을 그대로 유지한다.

## 실패 시 다음 행동

- Offline도 실패: 같은 records로 8 epoch까지 비교하고, 계속 실패하면 learning rate `0.0001`을 별도 실험한다.
- Offline 통과, closed-loop 실패: 동일 교사 trajectory만 더 반복하지 말고 모델 방문 상태 50,000개를 교사로 다시 평가하는 dataset aggregation을 구현한다.
- 둘 다 통과: 솔로 초기 모델을 고정하고, attack·garbage·B2B/Surge·상대 상태가 포함된 1대1 모델 단계로 이동한다.

설정의 기준값은 `configs/evaluation/solo_imitation_promotion_v1.json`에 고정되어 있다.
