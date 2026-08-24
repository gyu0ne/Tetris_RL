# Frame Timing 및 Rules Profile 설계 설명

상태: 기반 설계 및 첫 구현 완료
기준 시각: `2026-08-24T16:05:13+09:00`

## 1. 목적과 경계

이번 단계는 기하학적 배치 엔진 위에 결정론적 frame 전이를 추가하고, 그 전이에 들어갈 TETR.IO 값을 generic core와 분리하는 작업이다. 구현된 코드는 target mechanics를 확정했다고 주장하지 않는다. 현행 replay/config fixture로 확인하지 못한 수치는 실행 가능한 TETR.IO 기본값이 될 수 없다.

책임은 다음처럼 나뉜다.

- `engine-core::timing`: 주어진 수치와 이미 정렬된 action을 기계적으로 적용한다.
- `rules-tetrio`: upstream version, mode, 출처, 확인일, confidence와 fixture ID를 field별로 보존한다.
- 후속 `FrameNormalizer`: raw/held input을 DAS/ARR/DCD, IRS/IHS 및 동일 frame 충돌 규칙에 따라 ordered discrete action으로 바꾼다.
- 후속 replay/conformance 계층: target fixture와 frame별 state를 비교하고 최초 차이를 보고한다.

이 분리는 미확정 TETR.IO 입력 순서를 generic engine이 임의로 선택하는 것을 막는다.

## 2. 결정론적 시간 표현

중력은 `numerator / denominator` cell/frame의 유리수로 저장한다. 매 frame 분자를 정수 accumulator에 더하고, 분모 이상이 된 몫만큼 아래 이동한 뒤 나머지를 보존한다. 따라서 `0G`, `1/2G`, `1G`, `20G`를 authoritative transition에서 부동소수점 없이 재현할 수 있다.

`TimingState`는 다음 최소 상태를 가진다.

- active `PieceState`
- 중력 나머지 accumulator
- 현재 lock 경과 frame
- 사용한 lock reset 수
- lock 완료 여부

`step_frame(board, state, rules, ordered_inputs)`는 입력 적용, 중력, grounded 검사, lock timer 갱신 순서로 한 frame을 처리한다. hard drop은 즉시 floor까지 이동하고 lock한다. lock delay, lateral/rotation reset 허용 여부와 reset cap은 `TimingRules`의 explicit parameter다.

이 순서는 generic kernel contract이며 TETR.IO의 raw input 처리 순서와 동일하다는 주장이 아니다. target normalization과 단계 순서는 fixture를 확보한 뒤 versioned adapter 및 필요 시 kernel stage policy로 고정한다.

## 3. Rules profile 활성화 장벽

`TetrioRulesDraft::tetra_league_beta_1_7_8_season_2()`는 다음 identity를 고정한다.

- profile ID: `tetrio-beta-1.7.8-tetra-league-season-2`
- upstream: `BETA 1.7.8`
- mode: `TETRA LEAGUE Season 2`

각 field는 값, 단위, source URL, access date, confidence, fixture ID와 제한 note를 가진다. 공식 patch history가 확인한 version과 All-Mini+ 변경은 `CONFIRMED`, community 문서 기반 10×40/SRS+ 등은 `OBSERVED`, exact TL timing literal은 `UNCONFIRMED`다.

현재 draft에서 다음 6개 필수 timing field는 값이 없다.

1. gravity numerator
2. gravity denominator
3. lock delay frame
4. maximum lock reset 수
5. lateral move reset 여부
6. rotation reset 여부

`try_timing_rules()`는 하나라도 비어 있으면 누락 field 목록과 함께 실패한다. 따라서 historical 500 ms나 임의의 reset cap을 target default로 오인한 학습을 시작할 수 없다. unit test용 synthetic fixture profile은 활성화 경로 자체만 검증하며 TETR.IO 주장으로 사용하지 않는다.

## 4. 검증 결과

컨테이너의 Rust 1.89.0 toolchain에서 다음을 통과했다.

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- engine core 28개와 rules profile 3개, 총 31개 unit test
- `cargo build --workspace --release`

시험 범위에는 유리수 중력 누산, 20G floor 도달, lock 경계 frame, reset cap, hard-drop immediate lock, ordered input 결정론, 미확정 profile activation 거부와 provenance 보존이 포함된다.

## 5. 남은 작업과 통과 조건

다음 단계는 raw input normalizer, ARE/line-clear timing, last-action 및 kick metadata, spin/top-out, GameState lock 연결과 replay event다. target profile을 활성화하려면 사용자 소유의 pinned-version replay/config에서 exact literal과 frame order를 추출하고 각 field에 fixture ID를 연결해야 한다.

이 문서 단계의 완료는 “timing foundation이 generic parameter에 대해 결정론적”이라는 뜻이다. `TETR.IO BETA 1.7.8 / TL S2` mechanics conformance 완료를 뜻하지 않는다.
