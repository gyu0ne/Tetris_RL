# 사람 대 모델 로컬 대국

## 목적

이 도구는 현재 1대1 강화학습 checkpoint와 사용자가 직접 대국하기 위한 로컬 진단 화면이다. 별도 JavaScript 테트리스 구현이 아니라 프로젝트의 권위 Rust 엔진과 `BattleSession`을 그대로 사용한다. 따라서 화면은 단순해도 이동·회전·hold·lock·공격·상쇄·garbage·top-out·동시 사망 판정은 학습 환경과 같다.

기본 상대는 `checkpoints/versus-selfplay-r3/latest-model.pt`다. 서버를 시작할 때 모델을 한 번 메모리에 읽으며, 실행 중 checkpoint 파일이 바뀌어도 자동 재적재하지 않는다.

## 실행

```powershell
docker compose up --build human-battle
```

브라우저에서 `http://127.0.0.1:8789`을 열고 `시작`을 누른다. 종료는 다음 명령이다.

```powershell
docker compose stop human-battle
```

학습이 새 `latest-model.pt`를 만들었으면 서버를 재시작해 새 가중치를 읽는다.

```powershell
docker compose restart human-battle
```

다른 모델을 지정하려면 기존 서비스를 멈춘 뒤 호환되는 1대1 model checkpoint를 넘긴다.

```powershell
docker compose stop human-battle
docker compose run --rm --service-ports human-battle python -m tetris_rl.human_battle.server --checkpoint checkpoints/versus-selfplay-r3/update-000500-model.pt --bind 0.0.0.0:8789 --frames-per-placement 12 --threads 2 --allow-observed
```

`latest.pt`처럼 optimizer와 진행 경기까지 든 학습 재개 파일이 아니라, `latest-model.pt` 또는 `*-model.pt` 형식의 추론용 checkpoint를 지정한다.

## 조작

| 키 | 동작 |
|---|---|
| `←`, `→` | 좌우 이동 |
| `↓` | 소프트 드롭 |
| `Space` | 하드 드롭 |
| `Z`, `X` | 반시계·시계 회전 |
| `A` | 180도 회전 |
| `C` | hold |
| `P` | 일시정지·계속 |
| `N` | 일시정지 중 1프레임 진행 |
| `R` | 현재 입력 시드로 다시 시작 |

초기 화면은 정지 상태다. 시드를 바꾸고 `시드 적용`을 누르면 두 플레이어가 같은 새 piece seed로 재설정된다.

## 실행 구조

사람과 모델의 action 공간은 의도적으로 다르다.

- 사람: 브라우저의 key press/release를 프레임마다 Rust `InputEdge`로 전달한다. DAS, ARR, 회전, 낙하와 lock은 `FrameSession`이 처리한다.
- 모델: 학습 때와 똑같이 현재 상태에서 도달 가능한 `hold + orientation + x/y` 후보를 열거하고 actor logit이 가장 큰 후보를 고른다. 기본 cadence는 12 engine frame당 한 placement다.
- 대전 해결: 모델이 놓는 프레임에도 사람 입력을 함께 적용한 뒤, 양쪽 lock 결과를 한 번의 `BattleSession` transaction으로 해결한다. 어느 한쪽을 먼저 처리해서 생길 수 있는 공격 선후공 편향을 만들지 않는다.

즉 모델은 키 입력 경로 또는 finesse를 수행하지 않는다. 이 도구의 목적은 학습된 placement 정책의 실제 공격·방어·생존 능력을 사람이 체감하고 비교하는 것이며, 키보드 조작 속도를 학습한 모델과 겨루는 것이 아니다.

## 화면의 진단값

- `frame`: 현재 권위 대전 프레임
- `bot next`: 다음 모델 placement가 허용되는 프레임
- `candidates`: 직전 모델 결정에서 비교한 도달 가능 착지 수
- `inference`: 직전 actor 순전파 시간
- `checkpoint update`: checkpoint가 기록한 학습 update
- `parameters`: 로드된 actor-critic parameter 수
- 각 보드의 `블록`, `공격`, `GARBAGE`: 놓은 piece 수, 보낸 공격량, ready/pending garbage

## 성능과 한계

브라우저는 60 Hz로 다음 프레임을 요청하지만 HTTP 요청과 모델 추론이 끝난 뒤에만 다음 요청을 보낸다. 따라서 추론이 느리면 벽시계 기준 진행 속도는 60 FPS보다 낮아질 수 있다. 엔진 프레임의 순서와 결과는 건너뛰지 않으므로 mechanics 결과는 결정론적으로 유지된다.

이 화면은 로컬 단판 진단 도구다. 랭크, 매치 세트, TETR.IO UI 재현, 네트워크 지연 보정, 음향과 픽셀 동등성은 범위가 아니다.

## 구현 위치

- `crates/arena/src/human_battle.rs`: 사람/모델 혼합 cadence와 snapshot
- `crates/versus/src/battle.rs`: 사람 프레임 입력과 모델 placement를 같은 frame에서 해결하는 transaction
- `crates/py-bridge/src/lib.rs`: Python용 `HumanBattle` bridge
- `python/tetris_rl/human_battle/controller.py`: checkpoint 추론과 API 상태 조립
- `python/tetris_rl/human_battle/server.py`: 로컬 HTTP 서버
- `python/tetris_rl/human_battle/static/`: 키보드 입력과 대전 화면

## 검증 범위

Rust 테스트는 같은 프레임의 사람·모델 lock, 12프레임 cadence, snapshot 가시성을 확인한다. Python 테스트는 actor argmax 선택, reset, 잘못된 입력 거부를 확인한다. 브라우저 smoke 검증은 초기 정지, 실제 `Space` hard drop, 모델 결정과 작은 화면의 가로 overflow 부재를 확인한다.
