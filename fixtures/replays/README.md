# 리플레이 차분 검증 픽스처

## 왜 실제 리플레이가 필요한가

클라이언트 테이블은 설정값을 보여 주지만 같은 프레임의 키 입력, IRS/IHS, 중력, lock 및 spawn이 어떤 순서로 실행되는지는 증명하지 못한다. 사용자 소유 리플레이의 입력을 동일하게 재생하고 기준 클라이언트의 프레임 스냅샷과 로컬 엔진 스냅샷을 비교해야 이 순서를 확정할 수 있다.

## 입력 경계

- TETR.IO UI에서 사용자가 직접 저장한 `.ttr` 또는 `.ttrm`, 혹은 이미 보유한 리플레이만 사용한다.
- 인증 토큰이 필요한 내부 API, 라이브 경기 자동 조작 및 대량 수집은 사용하지 않는다.
- 공식 `tetrio-format-specs` 저장소는 현재 리플레이 스키마를 공개하지 않으므로 샘플 없이 wire format을 추측하지 않는다.
- 원본 파일은 `fixtures/replays/raw/`에 두며 Git에서 제외한다.
- 커밋 가능한 픽스처는 사용자 식별자를 제거하고 원본 SHA-256, client asset ID, 규칙 프로필, 정규화된 handling, 입력 edge와 기준 스냅샷만 보존한다.

## 비교 계약

`replay-conformance`는 기준 trace와 로컬 trace에서 최초로 다른 프레임을 반환한다. 비교 순서는 프레임 번호, 40개 보드 row bitmask, active piece, hold, preview, top-out, gravity/lock timing이다. 실제 `.ttrm` 어댑터는 사용자 제공 샘플의 구조를 확인한 뒤 별도 모듈로 추가한다.
