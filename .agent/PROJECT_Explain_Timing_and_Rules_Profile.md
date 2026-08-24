# Frame Timing, Handling 및 Rules Profile 설계 설명

상태: client-derived observed profile, generic handling normalizer 및 IRS/IHS 기반 구현 완료
기준 시각: `2026-08-24T16:57:14+09:00`

## 1. 목적과 책임 분리

이번 단계는 기하학적 배치 엔진에 결정론적 frame timing과 raw input normalization을 추가하고, TETR.IO room mechanics와 player-specific handling을 분리한다.

- `engine-core::timing`: 주어진 유리수 gravity와 ordered action을 적용하고 lock timer/reset을 갱신한다.
- `engine-core::handling`: ordered input edge와 held state를 generic DAS/ARR/DCD/soft-drop action 및 hold request로 바꾸고 spawn 시 held IRS/IHS를 샘플링한다.
- `rules-tetrio`: target version, mode, 값, 단위, 출처, confidence와 snapshot/fixture ID를 보존한다.
- `replay-conformance`: upstream wire format과 독립된 canonical frame snapshot을 비교해 최초 divergence를 반환한다.
- `FrameSession`: handling, timing, hold, lock/clear와 다음 spawn을 한 프레임 API로 연결한다.
- 후속 target adapter: 실제 player handling의 원본 단위 변환과 정확한 TETR.IO stage order를 replay fixture에서 공급한다.

generic normalizer의 순서는 결정론적 개발 contract다. exact TETR.IO stage order가 입증됐다는 뜻은 아니다.

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

`ActiveTimingProfile::gravity_at_frame`은 부동소수점 없이 이 schedule을 계산하고 20G에서 cap한다. 중력이 변해도 accumulator 단위가 바뀌지 않도록 전체 schedule에 공통인 고정 분모를 사용한다. target 값은 `1/50`을 `2400/120000`, 1초 증가 뒤 `47/2000`을 `2820/120000`, 20G를 `2400000/120000`으로 표현한다. margin frame 자체에는 초기 gravity를 유지하고 그 뒤 경과 frame에 따라 증가시키는 현재 generic contract는 replay boundary fixture 전까지 `OBSERVED`다.

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

`PlayerHandlingProfile`은 변환이 끝난 DAS/ARR/DCD/SDF를 프레임 단위로 보존한다. 원본 리플레이/UI 값의 단위 변환, fractional handling, OS event sampling과 exact target stage order는 아직 fixture-gated다. 현재 IHS→IRS 순서는 결정론적 generic contract이며 TETR.IO 인증값이 아니다.

## 6. 검증과 남은 작업

현재 unit suite는 initial/margin/increase/cap gravity, 30/15 lock profile, observed-vs-confirmed barrier, room handling 분리, player handling mapping, DAS/ARR/DCD 경계, ARR 0, sonic drop, held IRS/IHS와 IHS→IRS 적용을 검사한다. 추가된 session test는 hard drop 이후 lock/clear/next spawn 연결, 즉시 hold 순서와 여러 피스에 걸친 clone 결정성을 검사한다. `replay-conformance`는 동일 trace, 정확한 board row divergence, timing mismatch와 trace 길이 mismatch를 구분한다.

남은 핵심 작업은 다음과 같다.

1. exact target same-frame stage order 검증
2. last-action/kick metadata, spin/top-out
3. line/perfect-clear scoring과 solo profile
4. 충분한 기준 상태를 제공하는 fixture가 확보될 때 reference differential로 현재 `OBSERVED` 값을 `CONFIRMED`로 승격

이 설계 완료는 timing/handling 기반의 로컬 실행이 가능하다는 뜻이며, 전체 TETR.IO mechanics 동등성 완료를 뜻하지 않는다.
