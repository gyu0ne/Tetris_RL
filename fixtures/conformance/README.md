# 기능 동등성 fixture 계약

이 폴더는 TETR.IO 운영자의 승인이나 인증서를 보관하는 곳이 아니다. 버전이 고정된 TETR.IO 기준 실행과 로컬 엔진에 같은 초기 조건·입력을 주었을 때, 학습에 영향을 주는 관측 상태와 이벤트가 정확히 같은지 검증하는 익명화 fixture를 보관한다.

## 판정 원칙

- 판정 단위는 `FunctionalConformanceCase`다.
- 각 case는 target profile, 기준 client build, 수집 방법, 원본 artifact SHA-256과 mechanics claim을 가진다.
- solo trace는 board/garbage bit layer, active piece, hold, preview, typed top-out과 timing을 비교한다.
- battle trace는 양쪽 solo 상태에 더해 combo/B2B 상태, incoming packet 순서, sent lines, margin multiplier, frame event와 round result를 비교한다.
- 값은 허용 오차 없이 비교한다. target JavaScript의 부동소수점 누적이 mechanics인 경우에도 engine 내부의 audited compatibility 값 자체가 정확히 일치해야 한다.
- 하나라도 불일치하면 `Divergent`, 불일치는 없지만 필수 claim이 비어 있으면 `Incomplete`, 모든 claim이 하나 이상의 통과 case로 덮이고 전체 corpus의 불일치가 0개일 때만 `Conformant`다.
- `Conformant`는 선언한 fixture corpus에 대한 기능 동등성이고 TETR.IO 운영자의 승인이라는 뜻이 아니다.

## 필요한 기준 자료

리플레이 파일 자체는 필수가 아니다. 다음 중 하나면 된다.

1. 버전이 고정된 client 실행에서 같은 seed·room/player handling·입력을 적용하고 lock/clear/attack/garbage/terminal 경계의 상태와 이벤트를 추출한 capture
2. 해당 checkpoint를 포함하는 사용자 소유 replay export
3. 작은 custom-room 경계 사례를 수동 재현한 뒤, 입력과 결과를 독립적으로 두 번 확인한 capture

입력 이벤트만 있고 보드·공격·garbage 기준 상태가 없는 replay는 입력 parser 회귀에는 쓸 수 있지만 기능 동등성 case로는 세지 않는다. 화면을 재생하는 viewer는 필요하지 않다.

## corpus 작성 규칙

- 임계값은 가능한 한 `n-1`, `n`, `n+1`을 별도 case로 둔다.
- 7-bag 경계, 모든 kick index, lock/reset cap, opener 14, travel 20, cap 8, margin 시작 frame, Clutch 및 동시 사망처럼 학습 결과를 바꾸는 경계를 우선한다.
- 한 case가 여러 claim을 덮을 수 있으나, 실제 trace에서 그 mechanics가 발동하지 않았다면 claim을 붙이지 않는다.
- 원본 `.ttr`/`.ttrm`과 개인 식별 정보는 Git에 넣지 않는다. 익명화 normalized fixture만 원본 SHA-256과 함께 커밋한다.
- 내부 unit/property test는 회귀 방지 근거이지만 reference claim coverage로 세지 않는다.

현재 Rust 계약의 필수 claim 목록은 `replay_conformance::REQUIRED_MECHANIC_CLAIMS`가 단일 기준이다. 외부 adapter가 추가되면 adapter는 normalized trace를 이 crate의 `FrameSnapshot` 또는 `BattleSnapshot`으로 변환해야 한다.
