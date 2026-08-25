# 정규화 reference trace v1

상태: 설계·구현 완료
기준 시각: `2026-08-25T16:03:07+09:00`

## 목적

TETR.IO replay 포맷을 추측하거나 화면 재생기를 만들지 않고도, 버전이 고정된 기준 실행에서 추출한 상태·이벤트를 로컬 엔진과 exact diff할 수 있는 안정된 경계를 제공한다. 외부 extractor는 upstream 형식과 이 계약 사이만 책임지고, Rust engine은 upstream JSON 구조에 직접 의존하지 않는다.

## 파일 및 신뢰 경계

한 case는 manifest JSON과 trace JSON의 쌍이다. manifest는 target profile, reference build, 수집 출처, mechanics claim과 두 해시를 기록한다.

- `source_artifact_sha256`: 사용자가 보유한 원본 replay/capture를 식별한다.
- `trace_sha256`: adapter가 생성한 normalized trace의 정확한 byte를 고정한다.

loader는 trace 해시를 먼저 검증한 뒤 JSON을 해석한다. 두 해시를 하나로 합치지 않은 이유는 원본 provenance와 변환 결과 무결성이 서로 다른 주장이고, adapter 버전이 바뀌면 같은 원본에서 다른 normalized byte가 나올 수 있기 때문이다.

## canonical 변환

solo는 frame, 40개 occupancy/garbage row, active piece, hold, preview, typed top-out, fixed-point timing과 last action을 보존한다. battle은 양쪽 solo snapshot, combo/B2B, ordered incoming packets, sent totals, exact IEEE-754 margin multiplier, terminal result를 추가한다.

frame event는 lock/spawn 객체 전체를 복제하지 않는다. 그 결과는 같은 frame의 game snapshot에 이미 반영되므로, 일시적인 battle 차이를 찾는 데 필요한 attack outcome, cancellation, insertion, transmitted packet 순서만 `BattleEventsSnapshot`에 투영한다. 이로써 private engine transition 구조를 wire 계약에 고정하지 않으면서도 학습 reward와 상대 state를 바꾸는 정보를 잃지 않는다.

## 거부 조건

- schema version 또는 unknown JSON field
- 빈 provenance/claim, unknown·duplicate claim, malformed SHA-256
- 40행이 아닌 board, 10열 밖 bit, occupancy에 없는 garbage provenance
- 0-line/과다 attack packet, board 밖 garbage hole, non-finite/negative multiplier
- 비단조 frame, battle outer/player/event frame 또는 result 불일치
- `randomized_battle` 또는 battle-only claim을 solo trace로 위장한 case
- fixture 종류와 다른 local trace binding

허용 오차나 누락 필드 default는 없다. 형식이 변경되면 schema version을 올리고 별도 변환 경로를 추가해야 한다.

## 현재 한계와 다음 단계

`fixtures/conformance/examples`는 loader 회귀용 합성 자료라 claim coverage로 세지 않는다. 실제 TETR.IO 상태를 수집하는 extractor와 version-pinned reference corpus는 아직 없으므로 profile은 `OBSERVED_NOT_FUNCTIONALLY_VERIFIED`다. 다음 단계는 소량의 solo/TL 경계 capture를 이 형식으로 출력해 최초 divergence를 찾는 것이다.
