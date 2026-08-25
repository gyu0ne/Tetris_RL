# 정규화 schema 예제

`schema-example-solo-v1.*.json`은 loader 형식과 무결성 검사를 회귀 테스트하기 위한 합성 자료다. TETR.IO 실행에서 수집한 reference가 아니므로 기능 동등성 corpus나 mechanics claim coverage에 절대 포함하지 않는다.

실제 case는 같은 파일 쌍을 사용하되 다음을 모두 실제 값으로 바꿔야 한다.

- `target_profile`: 검증 대상 profile ID
- `reference_build`: capture 당시 TETR.IO build
- `source`: 익명화된 수집 경로와 adapter 버전
- `source_artifact_sha256`: 사용자 소유 원본 capture/replay의 SHA-256
- `trace_sha256`: 정규화된 `*.trace.json` 원본 byte의 SHA-256
- `claims`: 해당 trace에서 실제로 발동해 관측한 mechanics만 지정

manifest와 trace는 모두 schema version 1이다. JSON의 미정의 필드, 잘못된 행 폭·높이, board에 없는 garbage bit, 0-line packet, 유효하지 않은 enum, 순서가 감소하거나 중복된 frame은 loader가 거부한다.
