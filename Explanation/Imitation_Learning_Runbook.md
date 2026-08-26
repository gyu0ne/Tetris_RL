# 최종 솔로 모방학습 실행 절차

작성일: `2026-08-26`

이 문서는 짧은 smoke 모델이 아니라 1대1 강화학습의 초기 정책으로 사용할 장기 솔로 부트스트랩을 만드는 절차다. 권위 설정은 `configs/training/solo_imitation_bootstrap_v1.json`과 `configs/evaluation/solo_imitation_promotion_v1.json`이다.

최종 결과물은 다음 파일 하나다.

```text
checkpoints/solo-imitation-versus-bootstrap-v1/model.pt
```

## 한 명령 전체 실행

clean commit에서 다음을 실행하면 이 문서의 1~5단계를 순서대로 수행한다.

```powershell
./scripts/run-final-solo-bootstrap.ps1
```

기본값은 `balanced`다. PC 사용 상황에 따라 다음 셋 중 하나를 고를 수 있다.

| 프로필 | 독립 학습 병렬도 | 평가 자원 | 용도 |
|---|---:|---:|---|
| `light` | 1 worker × 2 threads | 2 threads | 다른 작업을 병행할 때 |
| `balanced` | 2 workers × 4 threads | 6 threads | 기본 권장값 |
| `max` | 3 workers × 5 threads | 10 threads | 학습 중 PC를 거의 사용하지 않을 때 |

```powershell
./scripts/run-final-solo-bootstrap.ps1 -ResourceProfile light
./scripts/run-final-solo-bootstrap.ps1 -ResourceProfile balanced
./scripts/run-final-solo-bootstrap.ps1 -ResourceProfile max
```

이 값은 `18 logical CPU / 8 GB RAM` 환경을 기준으로 정한 상한이다. gzip/JSON 해석 구간에는 CPU 사용률이 낮아질 수 있어 실제 사용률이 항상 표의 thread 합계와 같지는 않다. 프로필은 동시 run 수와 native thread 수만 바꾸며 배치 크기, 학습률, seed와 epoch 예산은 동일하다.

스크립트는 컨테이너를 build하고 100만 결정을 생성한 뒤 세 모델을 학습한다. 최종 생존 평가에서 top-out이 발생하면 25만 learner-state를 추가하고 세 모델을 처음부터 재학습하며, 이를 최대 두 번 수행한다. 라운드마다 선택·최종 평가 seed 집합도 바꾼다. 중간 데이터를 자동 삭제하지는 않는다.

이미 현재 dataset과 일부 seed 모델이 있다면 다음처럼 생성 단계를 생략하고 이어간다.

```powershell
./scripts/run-final-solo-bootstrap.ps1 -ResourceProfile max -ReuseDataset
```

`-ReuseDataset`은 records/manifest 존재 여부와 최소 결정 수를 먼저 확인한다. 이후 Python loader가 schema와 hash를 다시 검증한다. 완료된 `seed-2026.pt` 같은 후보는 dataset ID와 학습 설정이 맞을 때 재사용하며, 미완료 seed는 `seed-2027.progress.pt` 같은 파일의 다음 epoch부터 이어간다.

progress checkpoint는 매 epoch가 완전히 끝난 뒤 원자적으로 저장되며 다음을 포함한다.

- 현재 모델과 optimizer 상태
- validation 기준 best 모델과 best epoch
- early-stopping 누적 상태
- 완료 epoch와 전체 history

정상 완료되면 최종 `seed-*.pt`를 원자적으로 저장한 뒤 해당 progress 파일을 지운다. 중단 후 같은 명령을 다시 실행하면 된다. 프로필 변경은 허용되지만 dataset, batch 크기, 학습률, seed, early-stopping 설정 등이 달라지면 잘못 이어붙이지 않고 오류로 중단한다. 실행 중 소스 파일은 수정하지 않는다.

후보 선택을 통과하지 못하거나 learner-state 재학습 2회 후에도 top-out이 발생하면 비정상 종료한다. 성능 기준을 낮춰 약한 모델을 실사용 파일로 승격하지 않는다.

## seed의 의미

`base_seed`는 모든 게임에 같은 seed를 쓰겠다는 뜻이 아니다. 재현 가능한 seed 목록의 시작점이다. 게임 `i`의 실제 seed는 다음과 같다.

```text
seed(i) = 10001 + 104729 × i
```

따라서 4,096개 교사 게임은 모두 서로 다른 seed를 사용한다. `seed_stride=104729`도 manifest에 기록된다. 학습 후보는 서로 다른 초기화 seed `2026`, `2027`, `2028`로 세 번 독립 학습한다. 학습 데이터, 후보 선택용 게임, 최종 생존 게임은 서로 다른 seed 집합을 사용한다.

## 0. 소스 확정

```powershell
git status --short
git rev-parse HEAD
docker compose build rust
docker compose build training
```

본 데이터의 `engine_revision`은 실제 코드를 가리켜야 하므로 clean commit에서 시작한다.

### training 이미지에서 clippy 다운로드 오류가 날 때

PyO3 bridge 빌드는 `rustc`와 `cargo`만 필요하다. `Dockerfile.training`은 base image에 이미 설치된 `1.89.0` toolchain을 명시하므로 저장소의 개발용 `clippy/rustfmt` component를 추가로 내려받지 않는다.

오래된 revision에서 다음과 같은 오류가 발생했다면 최신 clean commit으로 이동한 뒤 전체 명령을 다시 실행한다.

```text
component download failed for clippy-x86_64-unknown-linux-gnu
dns error: failed to lookup address information
```

이 오류가 데이터 생성 전 training image build 단계에서 발생했다면 dataset이나 checkpoint는 만들어지지 않았으므로 별도 정리 없이 재실행해도 된다. 반면 crates.io package나 Python wheel 자체의 DNS 조회가 실패했다면 필요한 dependency가 아직 cache되지 않은 것이므로 네트워크가 복구된 뒤 같은 build를 재시도한다.

## 1. 교사 결정 최소 100만 개 생성

```powershell
$revision = (git rev-parse HEAD).Trim()

docker compose run --rm rust cargo run --release -p arena --bin generate-solo -- --records datasets/solo-imitation-bootstrap-v1/records.jsonl.gz --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --engine-revision $revision --seed 10001 --seed-stride 104729 --matches 4096 --decisions-per-match 250
```

확인한다.

```powershell
$manifest = Get-Content -LiteralPath 'datasets/solo-imitation-bootstrap-v1/manifest.json' -Raw | ConvertFrom-Json
$manifest | Select-Object decisions, requested_matches, base_seed, seed_stride, engine_revision
```

`decisions < 1000000`이면 `--matches 4224`로 전체 파일을 다시 생성한다. records는 학습과 offline 비교에만 필요한 임시 파일이다.

## 2. 세 초기화로 장기 학습

```powershell
docker compose run --rm training python -m tetris_rl.training.multiseed --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --output-dir checkpoints/solo-imitation-bootstrap-v2 --seeds 2026 2027 2028 --epochs 100 --min-epochs 20 --patience 10 --min-improvement 0.1 --shuffle-buffer 4096 --batch-decisions 64 --learning-rate 0.0003 --teacher-temperature 1.0 --teacher-score-scale 1000 --workers 2 --threads 4 --resume --allow-observed
```

`100 epoch`는 반드시 모두 실행한다는 뜻이 아니라 최대 예산이다. 각 run은 다음처럼 동작한다.

- 매 epoch에 bounded-memory shuffle로 데이터 순서를 바꾼다.
- 최소 20 epoch는 학습한다.
- validation mean teacher regret가 10 epoch 동안 의미 있게 개선되지 않으면 멈춘다.
- 마지막 epoch가 아니라 validation regret가 가장 낮은 epoch의 가중치를 저장한다.
- 세 run은 같은 데이터와 validation을 사용하므로 초기화 차이를 직접 비교할 수 있다.
- `--workers`는 동시에 실행할 초기화 run 수, `--threads`는 각 run이 사용할 native thread 수다.
- `--resume`은 호환되는 완료 후보와 epoch progress를 검증 후 재사용한다.

## 3. 후보 하나 선택

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.select --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --candidate checkpoints/solo-imitation-bootstrap-v2/seed-2026.pt --candidate checkpoints/solo-imitation-bootstrap-v2/seed-2027.pt --candidate checkpoints/solo-imitation-bootstrap-v2/seed-2028.pt --output-checkpoint checkpoints/solo-imitation-bootstrap-v2/selected.pt --offline-output runs/evaluation/solo-imitation-bootstrap-v2-offline.json --selection-output runs/evaluation/solo-imitation-bootstrap-v2-selection.json --base-seed 20001 --seeds 256 --horizon 2000 --batch-decisions 64 --threads 2 --min-dev-survival 1.0 --allow-observed
```

세 후보 모두 같은 validation records와 같은 256개 미사용 seed에서 비교한다. offline gate를 통과하고 2,000수 동안 한 번도 죽지 않은 후보만 선택 대상이다. 그중 생존율, 평균 생존 수, normalized regret 순으로 하나를 고른다.

## 4. 최종 대규모 솔로 생존 실행

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.closed_loop --checkpoint checkpoints/solo-imitation-bootstrap-v2/selected.pt --base-seed 40001 --seeds 2000 --horizon 10000 --threads 2 --allow-observed --require-gates --min-survival 1.0 --output runs/evaluation/solo-imitation-bootstrap-v2-final-closed-loop.json
```

이는 선택에 쓰지 않은 2,000개 seed에서 총 2,000만 placement를 실행한다. top-out이 한 번이라도 발생하면 통과하지 않는다. 이것이 이 프로젝트에서 사용하는 “혼자서는 죽지 않는 수준”의 운영상 정의다. 무한 시간과 모든 가능한 seed에 대한 수학적 증명은 아니지만, 짧은 1,000수 smoke와는 다른 장기 실주행 기준이다.

## 5A. 생존 통과 시 최종 승격

```powershell
docker compose run --rm training python -m tetris_rl.evaluation.promote --checkpoint checkpoints/solo-imitation-bootstrap-v2/selected.pt --offline-report runs/evaluation/solo-imitation-bootstrap-v2-offline.json --selection-report runs/evaluation/solo-imitation-bootstrap-v2-selection.json --closed-loop-report runs/evaluation/solo-imitation-bootstrap-v2-final-closed-loop.json --output checkpoints/solo-imitation-versus-bootstrap-v1/model.pt --allow-observed
```

승격기는 실제 `model.pt`와 세 보고서의 SHA-256, dataset ID, engine revision을 확인하고 평가 결과를 최종 체크포인트에 포함한다.

## 5B. 한 번이라도 죽으면 learner-state 재학습

모델이 실제로 방문한 상태 25만 개를 같은 휴리스틱 교사로 다시 라벨링해 기존 100만 records에 합친다.

```powershell
docker compose run --rm training python -m tetris_rl.training.aggregate --manifest datasets/solo-imitation-bootstrap-v1/manifest.json --checkpoint checkpoints/solo-imitation-bootstrap-v2/selected.pt --records datasets/solo-imitation-dagger-r1/records.jsonl.gz --output-manifest datasets/solo-imitation-dagger-r1/manifest.json --matches 1024 --decisions-per-match 250 --target-decisions 250000 --parallel-games 128 --threads 2 --allow-observed
```

그다음 2단계부터 manifest만 `datasets/solo-imitation-dagger-r1/manifest.json`로 바꿔 세 후보를 처음부터 다시 학습한다. 다시 죽으면 같은 방식으로 `dagger-r2`를 한 번 더 만든다. learner-state 집계 두 번 후에도 최종 생존 실행에서 죽으면 solo feature/model 용량을 재설계해야 하며 1대1 RL로 넘어가지 않는다.

## 6. 임시 파일 정리

최종 체크포인트 단독 로드가 성공한 뒤에만 datasets, 중간 후보, 외부 evaluation report를 삭제한다. 최종 `model.pt`에는 모델 구조, 정규화, 데이터·엔진 provenance, best epoch, 학습 history와 승격 보고서가 들어 있다.

## 다음 단계

이 결과는 최종 1대1 봇이 아니라, 장시간 생존 가능한 stack prior다. 다음 1대1 모델은 이 prior를 초기화에 사용하면서 attack, incoming garbage, B2B/Surge, 상대 보드와 round terminal을 추가해 self-play 강화학습한다.
