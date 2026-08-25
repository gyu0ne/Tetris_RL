# 기능 동등성 검증 설계

상태: 판정 방법론, Rust gate 및 normalized JSON v1 loader 구현 완료; 외부 reference corpus 미확보
기준 시각: `2026-08-25T16:03:07+09:00`

## 1. 용어 확정

이 프로젝트는 TETR.IO 운영자의 승인, 서명 또는 공식 인증을 요구하지 않는다. 목표는 학습에 영향을 주는 mechanics의 **기능 동등성**이다.

기능 동등성은 다음 명제로 정의한다.

> 같은 target profile, seed, 초기 board, player handling과 순서가 보존된 frame input을 주면, 기준 TETR.IO 실행과 로컬 엔진의 선언된 관측 상태·이벤트가 모든 검증 fixture에서 정확히 일치한다.

따라서 코드나 문서의 `Conformant`는 운영자 인증이 아니라 선언된 corpus에서 불일치가 0개라는 뜻이다. 알려지지 않은 비공개 내부 구현의 동일성까지 주장하지 않는다.

## 2. 세 겹의 검증

### A. 결정론·불변식 검증

unit/property test로 board 보존, 7-bag, line compaction, reachable action, attack/cancellation 보존, 동일 seed 재현과 zero-sum terminal을 확인한다. 이 계층은 구현 버그를 빠르게 찾지만 TETR.IO와의 동등성을 단독으로 입증하지 않는다.

### B. 경계 fixture 검증

client-derived 상수와 순서마다 최소 반례를 만든다. 임계값은 `n-1/n/n+1`, kick은 direct와 모든 fallback index, garbage는 packet·travel·cancel·cap·hole·insertion, battle은 동시 attack·Clutch·top-out을 포함한다. 각 fixture에는 target/build/source/SHA-256을 붙인다.

### C. differential corpus 검증

버전 고정 reference trace와 로컬 trace를 frame 순서로 비교한다. 최초로 다른 component를 반환하며 허용 오차는 없다. 많은 무작위 seed는 경계 fixture를 보완하지만 대체하지 않는다.

## 3. 구현된 판정 계약

`crates/replay-conformance`에 다음 계층을 추가했다.

- `BattleSnapshot`: 양쪽 `FrameSnapshot`, combo/B2B, ordered incoming garbage, sent lines, margin multiplier, battle frame event와 round result
- `compare_battle_traces`: 1대1 trace의 최초 불일치 component와 frame을 반환
- `MechanicClaim`: solo와 battle을 합친 20개 필수 mechanics coverage label
- `ReferenceEvidence`: target profile, reference build, 수집 출처, artifact SHA-256
- `evaluate_functional_conformance`: evidence 유효성, boundary/randomized trace 종류, exact diff, claim coverage와 기본 10,000개 randomized battle case 하한을 한 번에 판정

상태는 다음 셋뿐이다.

- `Incomplete`: 비교한 case에는 차이가 없지만 필수 claim 또는 10,000개 randomized battle case 하한이 비어 있음
- `Divergent`: 잘못된 evidence/case 또는 하나 이상의 exact mismatch가 있음
- `Conformant`: 필수 claim 전체가 통과 case로 덮이고 10,000개 randomized battle case와 supplied corpus 전체의 mismatch 0개를 만족함

attack·garbage·battle claim을 solo trace로 채우는 것은 코드에서 거부한다. 입력만 들어 있는 replay나 내부 unit test는 reference coverage로 승격하지 않는다.

## 4. 리플레이의 위치

리플레이 재생 기능은 필요 없다. `.ttr`은 seed, handling, ordered input과 기준 checkpoint를 운반하는 방법 중 하나일 뿐이다. 기준 client capture가 같은 정보를 제공하면 replay 없이도 검증할 수 있다.

현재 제공된 BLITZ `.ttr`은 입력과 handling 정보는 있지만 frame별 board/attack/garbage checkpoint가 없다. 따라서 parser 회귀 자료로만 유지하고 TL 기능 동등성 gate에는 포함하지 않는다.

## 5. 다음 실행 순서

`normalized JSON v1` loader는 구현되었다. manifest는 원본 capture/replay와 정규화 trace의 SHA-256을 별도로 보존하며, trace는 stable enum/field를 통해 `FrameSnapshot` 또는 `BattleSnapshot`으로 변환된다. unknown field와 구조·bit·packet·frame 오류는 load 단계에서 거부한다. event projection은 lock/spawn 결과를 board snapshot과 중복하지 않고 attack/cancellation/insertion/transmitted packet만 보존한다.

1. 버전 고정 capture extractor가 실제 TETR.IO 상태·이벤트를 normalized JSON v1로 출력하게 한다.
2. 작은 solo/TL custom-room boundary capture부터 필수 claim을 채운다.
3. 최초 divergence가 나오면 fixture를 최소화하고 engine 또는 profile을 수정한다.
4. 전체 필수 claim 및 randomized differential corpus가 0-diff가 되면 profile을 `FUNCTIONALLY_CONFORMANT`로 올린다.
5. 그 후 arena/heuristic teacher 기록 생성과 모방학습을 시작한다.

운영자 연락이나 승인을 기다리는 단계는 없다. 실제 blocker는 기준 상태·이벤트를 담은 version-pinned trace corpus뿐이다.
