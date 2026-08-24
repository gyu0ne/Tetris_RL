# 리플레이 차분 검증 픽스처

## 용도: 검증 전용

리플레이는 엔진 입력과 결과를 대조하는 검증 자료다. 사용자에게 보여 주는 replay player/viewer는 구현하지 않는다. headless test가 입력 이벤트를 엔진에 다시 적용해 예상 checkpoint나 최종 집계와 비교하는 것만 허용한다.

클라이언트 테이블은 설정값을 보여 주지만 같은 프레임의 키 입력, IRS/IHS, 중력, lock 및 spawn이 어떤 순서로 실행되는지는 증명하지 못한다. 다만 replay에 frame별 board checkpoint가 없다면 해당 파일만으로 매 프레임 mechanics를 인증할 수 없으므로, 형식·handling·입력 순서 회귀 자료로만 사용한다.

## 입력 경계

- TETR.IO UI에서 사용자가 직접 저장한 `.ttr` 또는 `.ttrm`, 혹은 이미 보유한 리플레이만 사용한다.
- 인증 토큰이 필요한 내부 API, 라이브 경기 자동 조작 및 대량 수집은 사용하지 않는다.
- 공식 `tetrio-format-specs` 저장소는 현재 리플레이 스키마를 공개하지 않으므로 샘플 없이 wire format을 추측하지 않는다.
- 원본 파일은 `fixtures/replays/raw/`에 두며 Git에서 제외한다.
- 커밋 가능한 픽스처는 사용자 식별자를 제거하고 원본 SHA-256, client asset ID, 규칙 프로필, 정규화된 handling, 입력 edge와 기준 스냅샷만 보존한다.
- 제공된 첫 fixture는 BLITZ이므로 TETRA LEAGUE versus rule 검증에 사용하지 않는다.

## 비교 계약

`replay-conformance`는 기준 trace와 로컬 trace에서 최초로 다른 프레임을 반환한다. 비교 순서는 프레임 번호, 40개 보드 row bitmask, active piece, hold, preview, top-out, gravity/lock timing이다. upstream adapter는 충분한 기준 상태가 있는 sample이 확보되고 실제 차분 검증에 필요할 때만 추가한다.
