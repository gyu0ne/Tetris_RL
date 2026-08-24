# Versioned rules records

이 폴더는 코드에 들어가는 규칙 literal의 사람이 검토 가능한 원장을 보관한다. 파일 이름에 target upstream version과 mode를 넣고, 각 값은 `CONFIRMED`, `OBSERVED`, `UNCONFIRMED` 중 하나로 구분한다.

- `OBSERVED` profile은 로컬 구현·실험에는 사용할 수 있지만 TETR.IO conformance 통과를 뜻하지 않는다.
- `CONFIRMED` 승격에는 기준 replay/config와 differential fixture ID가 필요하다.
- `room_handling = false`인 mode의 ARR/DAS/SDF는 player-specific input이며 mode 고정값으로 사용하지 않는다.
- client-derived record는 asset hash/version, 추출 시각, extractor revision과 snapshot hash를 함께 기록한다.
- solo score points는 record에 넣지 않는다. attack record는 1 대 1 terminal에 영향을 주는 base/combo/B2B/Surge/Perfect-Clear/garbage-clear 값과 ordered packet 규칙만 보존한다.
