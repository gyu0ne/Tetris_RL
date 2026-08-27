# 솔로 모델 관전자와 고속 평가

작성일: `2026-08-27`

이 구성요소는 승격된 실제 PyTorch checkpoint를 권위 Rust 엔진에서 실행해 사람이 착지 선택을 직접 관찰하고, 장기 closed-loop 평가를 여러 CPU 프로세스로 나누기 위한 도구다. 별도의 JSON 모델이나 JavaScript 테트리스 구현은 사용하지 않는다.

## 모델 관전자 실행

최종 모델이 다음 위치에 있어야 한다.

```text
checkpoints/solo-imitation-versus-bootstrap-v1/model.pt
```

실행한다.

```text
docker compose up --build spectator
```

브라우저에서 `http://127.0.0.1:8788`을 연다. 종료할 때는 다음을 실행한다.

```text
docker compose stop spectator
```

화면에서 할 수 있는 일은 다음과 같다.

- `재생`/`일시정지`: 모델이 선택한 착지를 연속 적용한다.
- `1수 진행`: 후보 생성→모델 채점→최고 점수 착지 적용을 한 번 수행한다.
- `재생 속도`: 사람이 보기 쉬운 간격부터 25수 묶음의 최대 모드까지 고른다.
- `시드 적용`: 같은 seed의 결정론적 게임을 처음부터 다시 만든다.
- `마지막 선택`: 후보 수, 선택 인덱스, raw model score, 전체 후보 점수 폭과 PyTorch 채점 시간을 표시한다.

표시 단위는 프레임이 아니라 학습 action인 `reachable locked afterstate`다. 따라서 이동·회전 경로 애니메이션은 없고 각 수의 최종 착지 결과가 즉시 나타난다. 이는 모델이 실제로 학습하고 평가받는 의사결정 단위와 같다.

## 데이터 흐름

```text
model.pt 로드
  → Rust SoloBatch가 hold 포함 도달 가능한 모든 착지를 열거
  → 10개 afterstate feature를 compact byte buffer로 Python에 전달
  → checkpoint 정규화 + 10→64→32→1 scorer로 모든 후보 채점
  → argmax index만 Rust로 반환
  → Rust GameState가 해당 착지를 적용
  → 20행 board snapshot을 관전자에 반환
```

Python/브라우저는 board legality, line clear, hold, spawn이나 top-out을 다시 구현하지 않는다. 브라우저는 서버가 반환한 occupancy bit row를 그릴 뿐이다.

## 추론 전용 Rust 후보 경로

교사 데이터 생성은 후보마다 teacher score, action token, path length, afterstate checksum과 immediate event를 기록해야 한다. 모델 추론에는 feature, placement와 hold 사용 여부만 필요하다. `SoloBatch.candidates()`는 이 세 값만 만드는 전용 경로를 사용하며 `labeled_candidates()`는 learner-state 집계에 필요한 기존 전체 라벨 경로를 유지한다.

두 경로가 동일한 seed에서 후보 순서와 feature를 정확히 보존하는 단위 테스트가 있다. 따라서 최적화는 모델이 보는 action 집합이나 선택 결과를 바꾸지 않는다.

## closed-loop seed 병렬 평가

평가 명령에 `--workers`를 추가할 수 있다.

```text
docker compose run --rm training python -m tetris_rl.evaluation.closed_loop --checkpoint checkpoints/solo-imitation-versus-bootstrap-v1/model.pt --base-seed 40001 --seeds 2000 --horizon 10000 --workers 6 --threads 2 --allow-observed --require-gates --min-survival 1.0
```

- `--workers`: 독립 seed shard 프로세스 수
- `--threads`: 각 worker가 쓰는 PyTorch native thread 수
- 총 계산 thread 상한의 근사값: `workers × threads`

각 게임은 seed만 공유하지 않는 완전 독립 상태이므로 shard 결과의 `survived`, 최소/최대 배치 수와 seed 가중 평균을 합치면 단일 프로세스와 같은 지표가 된다. 2026-08-27 스모크에서 동일한 24 seed×300수는 단일·4 worker 모두 24/24 생존과 평균 300수를 냈고, Docker 시작 시간을 포함한 wall time은 9.409초에서 6.865초로 줄었다. 짧은 작업은 프로세스 시작·모델 재로드 비용 때문에 이상적인 4배가 나오지 않으며, 장기 평가일수록 상대적 overhead가 작아진다.

## 자원 프로필

자동 스크립트는 다음 값을 사용한다.

| 프로필 | 평가 worker | worker별 thread | 근사 총 thread |
|---|---:|---:|---:|
| `light` | 1 | 2 | 2 |
| `balanced` | 3 | 2 | 6 |
| `max` | 6 | 2 | 12 |

메모리는 worker마다 checkpoint와 Python runtime을 별도로 가지므로 worker 수에 따라 증가한다. 이 모델은 2,817 parameter로 작지만 PyTorch runtime 자체 비용이 있으므로 18 logical CPU 환경에서 worker를 무제한 늘리지 않는다.

## 검증 범위

- Rust 전체 workspace test에서 추론/라벨 후보 feature와 순서 동등성을 확인한다.
- Python test에서 shard 지표의 seed 가중 합산과 관전자 controller의 실제 argmax 적용을 확인한다.
- 실제 승격 checkpoint로 `/api/state`와 3수 진행을 확인한다.
- 320/375/414/768px에서 가로 overflow와 두 줄 버튼 라벨이 없음을 브라우저로 확인한다.

관전자 자체는 모델 성능 gate가 아니다. 성능 판정은 기존 offline report와 독립 2,000 seed×10,000수 closed-loop 승격 보고서를 따른다.
