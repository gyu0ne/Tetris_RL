# Frame Session 연속 상태 전이 설계

상태: major design 및 첫 구현 완료
기준 시각: `2026-08-24T17:12:25+09:00`

## 1. 목적

기존 구현에는 `GameState`, `HandlingState`, `TimingState`가 각각 존재했지만 한 프레임 입력으로 여러 피스를 연속 진행하는 권위 있는 연결 계층이 없었다. `FrameSession`은 raw ordered edge를 받아 hold, 이동·회전, gravity, lock, line clear와 다음 spawn까지 하나의 결정론적 전이로 묶는다.

리플레이 파일은 이 API의 검증 입력이 될 수 있을 뿐 제품 기능이 아니다. 시각적 replay player/viewer는 설계에 포함하지 않는다.

## 2. 소유 상태와 API

`FrameSession`은 다음 상태를 소유한다.

- `GameState`: board, active piece, hold, next queue, bag과 placement count
- `Option<TimingState>`: 살아 있는 active piece의 gravity accumulator, lock timer와 reset count
- `HandlingState`: held key, DAS/ARR/DCD와 IRS/IHS buffer
- `frame: u64`: session-local frame index

`timing=None`은 더 진행할 active piece가 없는 terminal 상태를 뜻한다. 별도 boolean과 timing state가 서로 어긋나는 이중 상태를 만들지 않는다. `step`은 `SessionFrameOutcome`으로 정규화 입력, timing 결과, placement 결과, hold 적용, spawn 초기 입력과 terminal 여부를 반환한다.

## 3. 현재 generic stage order

한 프레임의 순서는 다음과 같다.

1. ordered input edge를 `normalize_frame`으로 정규화한다.
2. 즉시 hold 요청이 가능하면 현재 piece를 교체하고 timing state를 다시 만든다.
3. 정규화된 이동·회전·drop action과 gravity/lock을 적용한다.
4. lock되면 `GameState::lock_placement`가 board 기록과 line clear를 수행하고 다음 piece를 spawn한다.
5. 새 piece에 held IHS 후 IRS를 적용하고 다음 `TimingState`를 만든다.
6. top-out이면 timing을 제거하고 terminal로 종료한다.

이 순서는 독립 엔진 개발을 위한 generic contract다. exact TETR.IO same-frame order의 인증 주장이 아니며, target별 adapter와 differential fixture가 추후 순서를 공급하거나 제한한다.

## 4. 오류와 결정성

`SessionStepError`는 game transition 오류와 timing 오류를 구분해 보존한다. terminal session을 다시 step하면 `GameOver`를 반환한다. 모든 권위 상태는 정수·유리수 기반이고 wall clock이나 renderer를 읽지 않는다.

현재 test는 다음을 보장한다.

- hard drop 한 프레임이 lock과 다음 spawn까지 완료한다.
- hold가 같은 프레임의 timing action 전에 active piece를 교체한다.
- 동일 seed와 입력을 받은 clone 두 개가 여러 piece에 걸쳐 같은 outcome과 state를 유지한다.

## 5. 남은 conformance 작업

연속 상태 전이 연결은 완료됐지만 전체 mechanics는 아직 완료되지 않았다. 다음 단계는 last-action/kick metadata, All-Mini+ spin, perfect clear, exact top-out과 versus attack/garbage/round terminal이다. 충분한 기준 state가 있는 target fixture가 확보되면 generic stage order를 별도 target adapter로 검증한다.
