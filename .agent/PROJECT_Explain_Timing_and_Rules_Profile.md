# Frame Timing, Handling 및 Rules Profile 설계 설명

상태: client-derived observed timing mechanics 구현 완료, 외부 differential 인증 미완료
기준 시각: `2026-08-24T20:29:16+09:00`

## 1. 목적과 책임 분리

이번 단계는 기하학적 배치 엔진에 결정론적 frame timing과 raw input normalization을 추가하고, TETR.IO room mechanics와 player-specific handling을 분리한다.

- `engine-core::timing`: 주어진 gravity schedule을 client와 같은 1e-6 정수 microcell phase로 적용하고 lock timer/reset을 갱신한다.
- `engine-core::handling`: ordered input edge와 held state를 generic DAS/ARR/DCD/soft-drop action 및 hold request로 바꾸고 spawn 시 held IRS/IHS를 샘플링한다.
- `rules-tetrio`: target version, mode, 값, 단위, 출처, confidence와 snapshot/fixture ID를 보존한다.
- `replay-conformance`: upstream wire format과 독립된 canonical frame snapshot을 비교해 최초 divergence를 반환한다.
- `FrameSession`: handling, timing, hold, lock/clear와 다음 spawn을 한 프레임 API로 연결한다.
- 선택적 target adapter: 실제 player handling의 원본 단위와 0.1-subframe OS event를 replay fixture에서 공급한다.

frame-aligned edge는 기록된 순서를 보존하며, hold edge 앞뒤의 piece action도 분리한다. generic DAS/ARR/DCD 반복은 sandbox용 handling contract이고 개인 브라우저 입력의 인증값은 아니다. 주 학습 action은 reachable locked-afterstate이므로 raw OS event 반복은 학습 mechanics 동등성 gate에서 분리한다.

## 2. 2026-08-24 근거 갱신

독립 extractor `tetris-analyzes` commit `712dc10be43d5a6c54a35b62608ab9f4a2eaa324`의 freshness check를 container에서 실행했다. 저장돼 있던 2026-05-04 asset은 현재 asset과 달라 stale로 판정됐다. 이어서 현재 public client asset을 다시 추출했다.

| 항목 | 값 |
|---|---|
| 현재 asset | `63ab5c7c7.efa161fa8f91.20260810T191705` |
| 현재 generated snapshot SHA-256 | `7e0f6ba9ce214a9d2af226e35868031406c6553af4a040c9862ee5acec2aa217` |
| 이전 asset | `7eebfc9cd.987f91854aad.20260504T210001` |
| 이전 snapshot SHA-256 | `5935fc0e38f04b04bc1995bac9ce8715ea1e2bf3bb36bab0d4a1189a70c4e0fe` |
| 비교한 TL option | 31개 |
| 변경된 option | 0개 |

공식 FAQ는 DAS/ARR/DCD/SDF 의미를 설명하고, current Wiki는 500 ms lock과 move/rotation reset을 설명한다. client-derived 값과 서로 모순되지 않는다. 그러나 extractor가 공식 규칙 명세나 reference replay는 아니므로 값은 `OBSERVED`다.

## 3. 채운 TL timing 값

`configs/rules/tetrio-beta-1_7_8-tl-s2.observed.toml`과 `rules-tetrio`에 다음을 기록했다.

| field | 관찰값 | engine 표현 |
|---|---:|---:|
| tick rate | 60 Hz | 정수 frames/second |
| ARE | 0 frames | `u16` |
| line-clear ARE | 0 frames | `u16` |
| initial gravity | 0.02G | `1/50` cell/frame |
| gravity increase | 0.0035G/s | `7/2000` G/second |
| gravity margin | 7200 frames | 120 seconds |
| gravity cap | 20G | `20/1` cell/frame |
| lock delay | 30 frames | 500 ms at 60 Hz |
| lock resets | 15 | successful grounded move/rotation |

`ActiveTimingProfile::gravity_at_frame`은 exact rational schedule을 계산하고 20G에서 cap한다. client는 frame 끝에서 `frame > gmargin`일 때 증가시키므로 `margin + 1`의 action은 아직 초기 gravity를 보고, 첫 증가값은 `margin + 2`에서 적용된다. `TimingState`는 매 frame 이를 client의 1e-6 단위로 반올림한 뒤 정수 microcell phase에 더한다. 0~350,000 frame에서 client식 반복 `f64 +=`와 이 rational schedule을 각각 1e-6로 양자화한 값이 모두 같음을 회귀 검사한다. spawn은 occupancy ceiling 기준 `0.96`, fallback kick은 `0.1` phase에서 시작한다. 자동 낙하가 막힌 frame에만 lock counter가 증가하고 `locking > 30`에서 lock한다. 성공 move/rotation이 reset count 15에 도달하면 lock timer를 더 이상 지우지 않는다. 이 경계는 current bundle에서 직접 대조한 `OBSERVED`다.

## 4. 실행 가능과 conformance-ready의 분리

과거 문서의 “필수 timing field 6개가 비어 있어 activation 거부” 상태는 이번 조사로 대체됐다. 현재 profile은 모든 필수 timing literal이 있어 로컬 실행 가능하다.

그렇다고 conformance-ready는 아니다. `timing_conformance_blockers()`는 `CONFIRMED`가 아니거나 reference fixture가 없는 field를 계속 반환한다. 따라서 다음 두 명제를 구분한다.

1. `Executable OBSERVED`: client-derived 값으로 로컬 엔진·fixture 개발을 진행할 수 있다.
2. `CONFIRMED conformance`: pinned replay/config에서 exact state transition과 경계 frame이 일치한다.

학습 기록 생성은 관련 mechanics가 2번을 통과한 이후에만 승인한다.

## 5. Player handling 분리

TL option의 `room_handling=false` 때문에 함께 노출된 ARR 2, DAS 10, SDF 6은 effective TL 고정값이 아니다. 실제 reachability에는 각 player/replay의 handling profile이 들어가야 한다.

generic `HandlingState`와 `normalize_frame`은 다음을 구현한다.

- edge 순서를 보존하는 left/right/rotation/hard-drop action
- held direction의 DAS charge와 ARR 반복
- ARR 0의 board-width bounded instant shift
- rotation과 spawn의 DCD pause, 기존 DAS charge 유지
- finite soft drop과 sonic drop을 hard drop과 분리
- hold request를 timing action과 분리해 game layer에 전달
- held rotation/hold을 spawn에서 샘플링하고 generic IHS→IRS 순서로 적용

`PlayerHandlingProfile`은 변환이 끝난 DAS/ARR/DCD/SDF를 프레임 단위로 보존한다. 같은 frame의 edge 순서를 보존하며 rotate→hold와 hold→rotate가 서로 다른 결과를 낸다. 원본 replay/UI 단위와 0.1-subframe OS event sampling은 선택적 adapter가 맡는다. 개인 handling 반복의 세부 결과를 target 고정값으로 주장하지 않는다.

## 6. 검증과 conformance 경계

unit suite는 gravity schedule, spawn `0.96`과 kick `0.1` phase, `>30` lock boundary, reset cap, fallback kick 번호, same-frame hold 순서, IRS/IHS, last action, clear/spawn 연결과 clone 결정성을 검사한다. `replay-conformance`는 fall phase와 last action까지 비교한다.

선언한 학습 mechanics의 실행 구현은 완료됐다. 남은 작업은 충분한 기준 상태를 제공하는 외부 fixture가 확보될 때 `OBSERVED` 값을 differential 검증해 `CONFIRMED`로 승격하는 일이다. raw 개인 handling replay adapter는 필요할 때 별도 확장한다.
