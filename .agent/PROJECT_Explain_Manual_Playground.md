# 수동 mechanics 플레이그라운드

상태: solo 진단 도구 구현 완료
기준 시각: `2026-08-25T16:38:46+09:00`

## 목적

자동 fixture만으로 놓치기 쉬운 조작감과 핵심 상태 전이를 사용자가 직접 확인한다. TETR.IO 화면을 복제하는 도구가 아니며 pixel, animation, block skin, sound는 검증 대상이 아니다.

## 권위 경계

브라우저는 keydown/keyup 순서를 JSON edge로 보내고 10×20 visible board를 그리기만 한다. 이동, DAS/ARR/DCD, soft/hard drop, rotation/kick, hold, gravity, lock/reset, line clear, spin, perfect clear와 top-out은 `engine-core::FrameSession`이 계산한다. JS에 별도의 테트리스 규칙을 두지 않으므로 테스트 UI와 학습 엔진이 서로 다른 동작을 할 위험을 줄였다.

locked block은 현재 bitboard가 piece identity를 보존하지 않으므로 중립색으로 표시한다. garbage provenance와 active piece만 구분한다. 이는 mechanics 검증에 충분하며 색상 복제를 위해 engine state를 불필요하게 늘리지 않는다.

## 실행

`docker compose up --build playground` 후 `http://127.0.0.1:8787`을 연다. host port는 loopback에만 bind한다. 기본 handling은 테스트 사용자 설정 DAS 10, ARR 2, DCD 2, SDF 41이고 TL mode 고정값으로 취급하지 않는다.

## 현재 범위와 다음 확장

첫 도구는 solo session에 집중한다. 1대1 attack/garbage를 직접 검사하는 dual-board 화면은 같은 원칙으로 `BattleSession`을 연결해 다음 단계에서 추가한다. 이 도구의 수동 결과는 회귀 탐색에 유용하지만 formal reference conformance evidence를 대체하지 않는다.
