# Replay Conformance 및 IRS/IHS 설계 설명

상태: major design 및 engine-neutral differential foundation 구현 완료
기준 시각: `2026-08-24T16:57:14+09:00`

## 1. 실제 리플레이가 의미하는 것

여기서 reference replay는 사용자가 직접 저장하거나 이미 소유한 TETR.IO `.ttr`/`.ttrm` 파일과 그 경기의 기준 클라이언트 관찰 결과다. 라이브 서비스에 봇을 붙이거나 인증된 내부 API에서 대량 수집한다는 뜻이 아니다.

리플레이가 필요한 이유는 상수 확인과 transition 확인이 다르기 때문이다. client option에서 gravity나 lock 값을 읽어도 같은 frame의 input, spawn, IRS/IHS, gravity와 lock이 적용되는 순서는 알 수 없다. 동일 입력을 reference와 local engine에 적용해 최초로 다른 frame/state component를 찾아야 정확한 순서를 확정할 수 있다.

## 2. upstream format 경계

공식 `tetrio/tetrio-format-specs` 저장소는 확인일 현재 RSD sound spritesheet만 명시하고 replay wire format을 공개하지 않는다. 공식 issue tracker에는 raw `.ttrm` download endpoint 요청이 있지만 이는 2021년 사용자의 feature request이며 현재 형식 명세가 아니다.

따라서 다음 원칙을 적용한다.

- 실제 sample 없이 `.ttrm` key 이름이나 event 구조를 추측하지 않는다.
- 원본 replay는 `fixtures/replays/raw/`에서 Git 제외한다.
- sample을 받으면 asset/version에 고정된 adapter를 별도 모듈로 작성한다.
- 커밋되는 fixture는 사용자 식별자를 제거하고 원본 SHA-256과 provenance를 남긴다.

## 3. 구현된 canonical differential 계약

`crates/replay-conformance`의 `FrameSnapshot`은 다음을 보존한다.

- frame number
- 40개 board row bitmask
- active piece kind/orientation/origin
- hold와 preview
- top-out
- 선택적인 gravity accumulator, lock elapsed/reset count/locked state

`compare_traces`는 reference와 local trace를 순서대로 비교하고 최초 mismatch만 반환한다. mismatch는 frame number, 정확한 board row bit, active, hold, preview, top-out, timing 또는 trace length로 분류된다. 이 형식은 `.ttrm`에 종속되지 않아 adapter가 바뀌어도 engine comparison은 유지된다.

## 4. IRS/IHS 계약

`HandlingState`는 hold와 세 rotation key의 held state 및 최근 press 순서를 보존한다. spawn 시 `initial_actions_on_spawn`이 held IHS/IRS와 DCD를 샘플링한다. `GameState::apply_initial_actions`의 generic 순서는 다음과 같다.

1. hold가 가능하면 IHS 적용
2. hold 뒤 실제 incoming piece에 IRS 적용
3. kick 성공 결과와 top-out을 반환

이 순서는 독립 구현을 위한 명시적 provisional contract다. 실제 TETR.IO의 event sampling과 같은 frame 우선순위는 sample replay differential을 통과하기 전까지 `OBSERVED`나 `CONFIRMED`가 아니다.

## 5. Player handling

관측 TL profile은 `room_handling=false`이므로 reference fixture마다 effective player handling이 필요하다. `PlayerHandlingProfile` schema version 1은 변환 완료된 DAS/ARR/DCD와 soft-drop mode를 프레임 단위로 core에 전달한다. 원본 값과 단위를 확인하지 못하면 변환하지 않는다.

## 6. 완료 조건과 다음 작업

현재 구현은 differential harness의 engine 쪽 기반이 완료됐다는 뜻이다. target conformance 완료에는 다음이 더 필요하다.

1. user-owned current replay sample 1개와 가능한 경우 당시 handling export
2. sample 구조와 client asset을 기록한 upstream adapter
3. input event→canonical edge 변환 및 reference snapshot 생성
4. timing/game lock·clear transition 연결
5. boundary fixture와 full replay에서 unexplained divergence 0개

이 gate 전에는 current generic IHS→IRS 순서나 timing stage order를 TETR.IO와 동일하다고 주장하지 않는다.
