# TETR.IO 동등 엔진 및 강화학습 1 대 1 봇 통합 개발 계획서

> 기준일: 2026-08-24
> 현재 상태: Phase 1 deterministic core와 Phase 2 frame timing 기반 구현, target conformance 미완료
> 기준 문서: `.agent/RULE.md`, `.agent/PROJECT_Explain_*.md`
> 주의: 이 문서는 기존 영문 계획서를 한국어로 통합·번역한 설명서다. 아직 증거가 확보되지 않은 값은 `UNCONFIRMED`로 유지한다.

## 1. 프로젝트 목표

이 프로젝트의 최종 목표는 다음 두 가지다.

1. 공개적으로 검증 가능한 범위에서 특정 버전의 TETR.IO와 동일한 상태 전이와 타이밍을 재현하는 로컬 테트리스 엔진을 만든다.
2. 이 엔진 위에서 제한된 연산 자원을 효율적으로 사용해 강한 테트리스 1 대 1 강화학습 봇을 학습하고 평가한다.

초기 기준 프로필은 잠정적으로 `TETR.IO BETA 1.7.8 / TETRA LEAGUE Season 2`로 고정한다. 다만 실제 replay와 설정 export를 확보하기 전까지 정확한 TETRA LEAGUE 상수는 확정하지 않는다. 이후 TETR.IO가 업데이트되더라도 이 프로젝트의 기준 버전은 자동으로 바뀌지 않는다.

## 2. “TETR.IO와 완벽히 같음”의 정의

TETR.IO의 서버 및 내부 소스 코드는 공개되어 있지 않으므로, 독립 구현이 내부 코드까지 완전히 동일하다고 증명하는 것은 불가능하다. 따라서 이 프로젝트에서 동등성은 다음과 같은 **관찰적 동등성**으로 정의한다.

> 고정된 규칙 프로필, seed, 초기 상태, 프레임 단위 입력 순서가 같을 때, 검증 fixture가 포괄하는 모든 상황에서 로컬 엔진이 기준 TETR.IO와 동일한 관찰 가능 상태와 이벤트를 생성해야 한다.

여기서 동등성은 **학습 가능한 전략, legal action, 상태 전이 또는 terminal reward에 영향을 주는 mechanics**로 한정한다. 내부 코드, UI나 서비스 시스템 전체의 복제를 뜻하지 않는다. 다음 항목이 일치해야 한다.

- tetromino 형태, spawn 위치, hold 및 next queue
- seed 기반 7-bag과 piece 순서
- SRS+ 회전, kick, 180도 회전 및 이동 가능성
- DAS, ARR, DCD, SDF, gravity, lock delay와 reset
- line clear, spin, perfect clear와 모드별 점수
- 공격량, combo, B2B Charging/Surge와 상쇄
- garbage 전송, 이동 중 상태, cap, messiness, 삽입 및 hole
- top-out, 동시 사망, Clutch Clear 및 round terminal
- 입력·이벤트·state hash를 포함한 결정론적 replay

계정, rating, 매치메이킹, lobby, 공식 서버, 외형, UI, 음향, 안티치트 및 공개되지 않은 네트워크 구현은 엔진 동등성 범위에 포함하지 않는다. match 사이의 progression도 학습 목적에 필요하지 않으면 제외한다. 반면 round 승패를 바꾸는 mechanics는 terminal reward 때문에 포함한다. 네트워크 지연은 로컬 arena의 명시적인 실험 변수로 모사할 수 있지만, 이를 TETR.IO의 비공개 서버 동작을 복제한 것이라고 주장하지 않는다.

애매한 기능은 다음 영향 시험으로 결정한다. legal action/reachability, 공개 observation/state transition/RNG, attack/garbage timing, round terminal 중 하나를 바꾸면 포함한다. 단지 학습·평가 편의나 화면 표현만 바꾸면 동등성 대상이 아니다.

## 3. 프로젝트 전체 운영 규칙

### 3.1 권한과 범위

- 사용자 지시가 저장소 규칙보다 우선하며, 충돌과 처리 결과는 `.agent/CONTINUITY.md`에 기록한다.
- 이 프로젝트는 독립적인 로컬 엔진과 봇 대전 환경이다.
- 실제 TETR.IO 서비스에 자동 접속하거나, 입력을 주입하거나, 봇 플레이를 수행하지 않는다.
- 공개되지 않은 서버 동작은 추측하지 않고 `UNCONFIRMED`로 표시한다.
- GitHub와 Reddit의 설명은 공식 근거가 아니라 검증할 가설 또는 edge case 후보로 취급한다.

### 3.2 변경 및 연속성 관리

- 매 작업 시작 시 `.agent/CONTINUITY.md`를 먼저 읽는다.
- 의미 있는 생성, 수정, 이동, 삭제, 의존성 변경, 규칙 변경, 설계 결정, 검증 실패와 프로젝트 전체 영향은 이유 및 근거와 함께 연속성 파일에 기록한다.
- 각 항목에는 ISO 형식 시각과 `[USER]`, `[CODE]`, `[TOOL]`, `[ASSUMPTION]` 중 하나의 태그를 붙인다.
- 알 수 없는 내용은 추정하지 않고 `UNCONFIRMED`로 기록한다.
- 큰 설계가 승인되거나 완료되면 `.agent/PROJECT_Explain_<세부제목>.md`를 작성하거나 갱신한다.
- 작은 patch 단위로 수정하며 관련 없는 사용자 변경을 보존한다.
- 생성 데이터, checkpoint, cache 및 benchmark 결과는 명시적 승인 없이 저장소에 commit하지 않는다.

### 3.3 컨테이너와 재현성

- 개발 도구와 의존성은 기본적으로 컨테이너에서 실행한다.
- host 운영체제에 system package를 직접 설치하지 않는다.
- 모든 실험은 commit된 설정과 container image digest로 재현할 수 있어야 한다.
- seed, 코드 revision, config hash, hardware, wall time과 신뢰구간을 함께 보고한다.

## 4. 저장소 구조

기능별 책임을 다음과 같이 분리한다.

```text
crates/
  engine-core/       # 결정론적 board, piece, 이동, timing
  rules-tetrio/      # 버전별 TETR.IO 규칙 및 mode profile
  versus/            # attack, garbage queue, round-terminal mechanics
  replay/            # 결정론적 event log, 재생, fixture
  arena/             # 로컬 bot 대 bot 실행 환경
  bot-protocol/      # TBP 호환 adapter와 로컬 protocol
  py-bridge/         # vectorized simulation을 위한 최소 Python binding
python/tetris_rl/
  envs/              # Gym 형식 batched environment
  features/          # 검증된 feature와 실험 feature
  models/            # policy/value model
  training/          # self-play, opponent pool, checkpoint
  evaluation/        # rating, ablation, 통계 보고서
configs/             # 변경 불가능한 버전별 규칙 및 실험 설정
tests/
  unit/ property/ differential/ replay/ golden/ fuzz/
benchmarks/          # 재현 가능한 engine/training 성능 측정
research/            # 실험 manifest와 생성 보고서
.agent/              # 규칙, 연속성, 계획 및 설계 설명
```

렌더링/UI, 게임 규칙, 학습 코드 및 실험 결과를 한 모듈에 섞지 않는다.

## 5. 전체 시스템 아키텍처

```text
UI / CLI / 실험 실행기
          |
     로컬 arena  <----> TBP adapter
          |
 버전이 고정된 versus rules
          |
   결정론적 engine core
          |
 replay/event log + golden fixture
          |
 native batched simulator bridge
          |
 Python environment -> policy/value 학습 -> 평가기/opponent pool
```

권위 있는 엔진 동작은 다음 순수 상태 전이로 표현한다.

```text
next_state = step(state, ordered_frame_inputs, rules, seed)
```

렌더러, wall clock, socket 및 학습기는 typed command를 통하지 않고 엔진 상태를 직접 변경할 수 없다.

### 5.1 핵심 표현

- **Board:** 행 중심 bitboard. 최소 40개의 논리적 행과 visible/hidden 경계를 명시한다. 너비 mask로 충돌 및 full row 검사를 bit 연산으로 처리한다.
- **시간:** 정수 frame/tick과 문서화된 60Hz 변환을 사용한다. 필요한 subframe 순서는 float가 아닌 fixed-point sequence로 표현한다.
- **Piece:** 종류, 회전 상태, 원점, lock/reset counter, 마지막 성공 이동·회전 및 spin 판정에 필요한 kick index를 저장한다.
- **Queue:** RNG 알고리즘, seed, bag 상태, hold 상태와 보이는 next 개수를 명시한다.
- **Garbage:** 송신자, 양, hole seed/column, 도착·활성화 frame, 상쇄 상태 및 profile별 flag를 가진 순서 있는 packet으로 표현한다.
- **Replay:** rules hash, seed, 초기 상태, 순서 있는 입력·이벤트, 주기적 state hash와 최종 결과를 포함한다.

### 5.2 규칙 프로필

1. `modern-core`: 10열 field, tetromino geometry, hold, 7-bag, SRS+/180도 회전, frame 처리
2. `tetrio-beta-1_7_8-tl-s2`: Season 2 1 대 1 All-Mini+, 공격, Surge, opener, garbage 및 round-terminal 규칙
3. `40-lines`, `blitz`, `zen-custom`: 각 모드의 gravity, score, 목표 및 사용자 설정
4. `quick-play-royale`: 높이 배수, targeting, mod 등 1 대 1과 다른 동작 때문에 별도 후속 profile로 분리

fixture로 입증되지 않은 필수 값은 임의의 기본값으로 채우지 않으며, 해당 profile을 사용 불가 상태로 둔다.

### 5.3 필수 인터페이스

- `RulesProfile`: 변경 불가능하고 versioned되어야 하며, 필요한 근거 기반 필드가 없으면 생성을 거부한다.
- `Engine::step`: 동일 입력에서 항상 동일 결과를 내야 한다.
- `Engine::legal_afterstates`: 입력 planner와 같은 reachability 판정을 사용한다.
- `Replay::verify`: 처음 달라지는 frame과 state component를 보고한다.
- `VectorEnv::step_batch`: Python GIL을 해제하고 cell별 Python 객체 없이 packed observation을 반환한다.
- `Arena::match`: seed, rules hash, latency model, 시간·node budget과 opponent ID를 명시적으로 받는다.

## 6. 엔진과 학습 시스템의 경계

각 piece 의사결정 시 native engine이 hold 분기를 포함한 모든 도달 가능한 고정 상태, 즉 **reachable locked afterstate**를 열거한다. 학습기는 가변 길이 action set의 각 후보를 평가해 하나를 선택한다.

선택 후 결정론적 movement planner가 정확한 frame 입력 sequence로 변환하고, engine이 실제 도달 가능성을 다시 검사한다. 이 방식은 다음 문제를 줄인다.

- 학습기가 불가능한 배치를 악용하는 문제
- key 단위 장기 credit assignment에 학습 자원을 낭비하는 문제
- 학습 환경과 실제 실행 환경의 이동 판정이 서로 달라지는 문제

frame 단위 직접 조작은 향후 oracle movement planner를 모방하거나 RL로 학습하는 2단계 문제로 추가할 수 있다. 첫 번째 강한 전략 봇의 필수 조건은 아니다.

## 7. 기술 선택

- 엔진, arena, replay, protocol, fuzzing 및 Python extension은 Rust workspace로 구현한다.
- 실험 실행과 모델 학습은 Python 및 PyTorch를 사용한다.
- 규칙과 실험 설정은 사람이 검토할 수 있는 JSON/TOML로 관리한다.
- config의 canonical hash를 replay와 checkpoint에 저장한다.
- toolchain과 release build는 container로 재현한다.

Rust는 결정론적 저수준 제어, bit 연산, 안전한 병렬 simulation 및 Cold Clear와 같은 기존 versus bot 생태계와의 호환성 때문에 선택한다. Python은 correctness kernel 밖에서 연구 반복 속도를 높이는 용도로만 사용한다.

다음 shortcut은 사용하지 않는다.

- 하나의 Python grid engine에 게임과 학습을 모두 구현
- 근거 없이 pixel-only CNN과 raw key action을 기본값으로 선택
- 팬 제작 공격 공식을 현재 TETR.IO의 확정 규칙으로 복사
- conformance fixture를 통과하기 전에 학습 시작

## 8. TETR.IO 규칙 동등성 검증 전략

### 8.1 근거 우선순위

1. TETR.IO 공식 patch note와 기준 client/replay가 export한 데이터
2. maintainer 설명과 이를 인용한 최신 TETR.IO Wiki
3. 통제된 custom room에서 반복한 black-box fixture
4. 독립 open-source 구현 및 parser
5. Reddit 등 플레이어 기록

근거가 충돌하면 우선순위가 높은 자료를 따른다. 자료의 작성 시점이나 game version이 다른지 먼저 확인한다.

모든 규칙 상수에는 다음 정보를 기록한다.

- source URL과 access date
- upstream version
- 값과 단위
- 신뢰도: `CONFIRMED`, `OBSERVED`, `UNCONFIRMED`
- 해당 값을 검증한 fixture ID

### 8.2 현재 고정 가능한 사실

- Season 2는 Beta 1.2.0에서 시작했다.
- 기존 B2B 동작은 B2B Charging/Surge로 변경되었다.
- Season 2 시작 당시 All-Mini, 첫 14개 piece 조건의 초기 상쇄 보정 및 All Clear 공격량 5가 도입되었다.
- Beta 1.5.0에서 multiplayer 기본이 All-Mini+로 바뀌었고, 3-corner를 충족하지 못하지만 immobile인 T-spin의 Mini 판정과 reworked Clutch Clear가 추가되었다. 현재 target은 All-Mini+다.
- Beta 1.3.0은 multiplier 영향을 받지 않는 garbage clear difficult-clear flat `+1`을 추가했다.
- 기본 회전은 SRS+이며 SRS-X는 별도 선택지다.
- multiplayer piece 생성은 seed가 있는 7-bag이다.
- 커뮤니티 protocol 문서가 보고한 입력 순서 `ZLOSIJT`는 replay fixture로 확인하기 전까지 `OBSERVED`다.
- current-client 추출에서 TL timing option은 `g=0.02`, `gincrease=0.0035`, `gmargin=7200`, ARE/line-clear ARE 0, lock time 30, lock resets 15로 두 asset snapshot에 걸쳐 동일했다. 이 값은 `OBSERVED`이며 reference replay 검증 전에는 `CONFIRMED`가 아니다. garbage cap/messiness, frame order, top-out과 mechanics 관련 round parameter는 여전히 `UNCONFIRMED`다.

### 8.3 Fixture 행렬

각 항목은 최소 replay, 예상 event와 state hash를 가진 실행 가능한 test로 바꾼다. 모든 임계값은 `n-1`, `n`, `n+1` 경계를 검사한다.

- 7종 piece spawn, 4방향, 양쪽 wall, floor/ceiling, 모든 kick 및 180도 회전
- hold-empty, hold-swap, piece당 1회 제한, IRS/IHS 순서, next, bag 경계와 seed replay
- DAS charge, ARR 0/비0, 방향 변경, DCD, SDF, gravity, lock delay, reset 수, sonic/hard drop, ARE 및 line-clear delay
- single/double/triple/quad, T mini/full spin, All-Mini+ non-T/T immobility, last-action/kick-index edge case, perfect clear
- combo, B2B 유지·중단, Surge charge/release/split, 첫 14개 piece 상쇄 경계, garbage-clear bonus
- garbage transit, zero passthrough, 상쇄 순서, cap, packet 경계, hole 반복·변경, 삽입·활성화와 lethal garbage
- block-out, lock-out, partial lock-out, simultaneous death, Clutch Clear/out-of-bounds 동작, round terminal
- 40 LINES와 BLITZ의 score, gravity와 종료, custom option serialization, 이후 QUICK PLAY 별도 suite

### 8.4 Differential harness

사용자가 소유한 replay/config export만 입력으로 사용하며 private API를 scrape하거나 live match를 제어하지 않는다.

```text
fixture -> normalize -> local replay -> frame별 state hash
                         |                 |
reference checkpoint ----+------- diff ----+
```

reference가 직접 노출하지 않는 내부 상태는 가장 이른 후속 관찰 결과와 비교하며 신뢰도는 `OBSERVED`로 둔다. 별도의 property test는 occupied cell 보존, line clear 압축, bag permutation, seed replay 결정론, 공격·상쇄 보존과 zero-sum 결과를 검사한다.

### 8.5 동등성 통과 게이트

- **C0:** 설정된 모든 상수에 source와 confidence가 있다.
- **C1:** core geometry, RNG와 input timing fixture가 100% 통과한다.
- **C2:** clear, spin과 attack fixture가 100% 통과한다.
- **C3:** garbage 및 round-terminal fixture가 최소 10,000개의 seed 기반 무작위 differential case에서 100% 통과한다.
- **C4:** 전체 replay corpus에서 설명되지 않은 차이가 0개다.
- **C5:** debug/release build와 지원 platform 사이에서 결정론 결과가 동일하다.

여기서 100%는 선언된 fixture corpus에 대한 수치다. 알 수 없는 hidden behavior 전체를 완벽히 포괄한다는 뜻은 아니다. 결과와 함께 coverage 및 confidence를 공개한다.

## 9. 강화학습 문제 정의

TETRA LEAGUE 한 round를 seed가 있는 2인 zero-sum 확률 게임으로 본다. piece와 hold 분기가 주어질 때 전략적 의사결정을 수행한다.

관측에는 다음 공개 정보만 포함한다.

- 양쪽 visible board
- current, hold 및 next piece
- incoming garbage packet과 timing
- combo, B2B, Surge 상태
- piece 수와 round-terminal context
- legal afterstate 목록

숨겨진 RNG 내부 상태는 policy에 제공하지 않는다.

### 9.1 CNN을 기본값으로 선택하지 않는 이유

고전 Tetris 연구는 compact structural feature와 afterstate evaluation만으로도 강한 결과를 보였다. 최근 bitboard 연구는 simulator 가속 및 afterstate actor의 효율을 보고했고, Cold Clear는 현대 versus 환경에서 search와 evaluator의 실용성을 보여준다.

따라서 raw 2D CNN은 관습적으로 채택하지 않는다. 동일한 wall time과 inference budget에서 실제 성능 우위를 증명해야 한다.

모델 실험 순서는 다음과 같다.

1. 정규화한 Dellacherie/Thiery 계열 및 versus feature를 사용하는 선형 evaluator
2. 모든 candidate afterstate에 공유되는 소형 MLP
3. 2번보다 단위 wall time당 성능이 높을 때만 compact spatial encoder와 scalar context를 결합한 hybrid 모델
4. 가장 좋은 scorer가 유도하는 얕은 beam/MCTS search

후보 feature는 landing height, eroded cell, row/column transition, hole와 hole depth, cumulative well, bumpiness, reachable garbage well, attack/cancel, board danger, clean/dirty incoming, B2B/Surge 기회 및 상대 기준 차이 feature다. T-spin pattern feature는 실제 reachability 근거를 통과해야 한다.

주요 학습 후보는 legal afterstate만 입력받는 공유 scorer, actor softmax와 중앙집중식 학습 value head의 조합이다. 양쪽 player가 parameter를 공유하며 player swap antisymmetry를 사용한다. inference 시 privileged state는 제거한다.

PPO는 2026년 bitboard/afterstate 연구가 sample efficiency를 보고했기 때문에 후보에 포함하지만, noisy cross-entropy/ES 및 승인된 heuristic-imitation 초기화보다 이 프로젝트의 versus benchmark에서 우수하다는 결과가 나와야 최종 채택한다.

## 10. 보상 함수와 수학적 검증

### 10.1 원래 목적

player 1의 shaping 전 보상은 다음과 같다.

```text
r_t = +1  승리
r_t = -1  패배
r_t =  0  무승부 또는 비종료 전이
r_t(player 2) = -r_t(player 1)
```

공격량, 높이, hole, 생존 시간 또는 APM을 직접 보상하지 않는다. 이런 항은 공격 farming, 고의 지연 또는 승률을 떨어뜨리는 suicidal spike를 유도해 실제 승패 목적을 바꿀 수 있다.

### 10.2 허용하는 dense shaping

허용되는 dense term은 potential difference 형태뿐이다.

```text
F(s, a, s') = gamma * Phi(s') - Phi(s)
r'_t = r_t + lambda * F(s, a, s')
```

Ng, Harada, Russell의 결과에 따라 이 형태는 정리의 조건을 충족하는 MDP에서 최적 policy를 보존한다. 이 프로젝트는 2인 stochastic game이므로 Lu, Schwartz, Givigi의 potential-based reward transformation 아래 Nash-equilibrium invariance 결과를 직접 이론 근거로 삼고, 다음 antisymmetric potential을 사용한다.

```text
Phi(s) = clip(w^T [f(self, s) - f(opponent, s)], -1, 1)
Phi(s_terminal) = 0
Phi(swapped_players(s)) = -Phi(s)
```

`gamma = 1`이고 terminal potential이 0이면 shaping 보상의 합은 고정 초기 상태에서 `-Phi(s_0)`로 telescoping된다. 이는 policy와 무관한 상수다. `gamma < 1`에서도 discounted sum이 같은 방식으로 telescoping된다.

이 수학적 결과는 모델링한 state/action 게임에서 목적이 바뀌지 않음을 보이는 것이며, 근사 신경망의 학습 속도 향상을 자동으로 증명하지는 않는다.

### 10.3 각 보상 feature의 검증 절차

각 feature `f_i`는 nonzero weight를 받기 전에 다음을 모두 통과해야 한다.

1. **정의 증명:** 단위, 범위와 정규화를 결정론적으로 정의하고 property test로 boundedness를 확인한다.
2. **대칭성 증명:** player swap 시 potential 부호가 바뀌고 terminal potential이 0인지 검사한다.
3. **policy/equilibrium invariance 검사:** 축소 board finite stochastic game을 전수 열거하고 shaping 전후 minimax/optimal action 및 Nash-equilibrium 집합이 exact arithmetic에서 같은지 확인한다.
4. **한계 효과 실험:** 동일 seed와 opponent sample에서 one-feature-at-a-time 및 leave-one-out ablation을 수행한다.
5. **작동 메커니즘 측정:** 승률과 함께 holes, danger integral, attack conversion, cancellation efficiency, gradient signal-to-noise와 episode length를 측정한다.
6. **통계 검증:** paired bootstrap confidence interval과 순차 비교 보정을 사용하며, 사전 선언한 실용적 임계값을 신뢰구간이 넘는 효과만 채택한다.

weight는 training opponent에서 선택한 뒤 held-out opponent/time-control suite에 대해 고정한다. 각 feature가 신경망 학습 속도에 미치는 영향을 닫힌 형태의 수식으로 완전히 증명할 수 있다고 과장하지 않는다.

## 11. Self-play 및 평가

### 11.1 휴리스틱 기록 기반 초기화

강화학습 전 여러 linear/search teacher가 생성한 기록으로 afterstate scorer와 value head를 pretrain한다. 선택 행동 하나만 복제하지 않고 legal candidates 전체의 score/rank와 teacher margin·style·budget을 저장한다. 최초 behavior cloning 뒤에는 learner가 실제로 방문한 state에 teacher를 다시 질의하는 dataset aggregation을 수행하고, 마지막에는 terminal 승패 목적의 self-play RL로 teacher ceiling을 넘도록 한다.

정식 dataset은 target mechanics conformance 뒤에만 생성한다. solo heuristic 기록은 board representation bootstrap에만 사용하며 최종 initialization에는 attack, garbage와 opponent context가 있는 1 대 1 기록이 필요하다. 상세 결정은 `.agent/PROJECT_Explain_Imitation_Bootstrap.md`와 `research/IMITATION_BOOTSTRAP_RESEARCH_KO.md`를 따른다.

- current, historical, exploit 및 baseline policy를 확률적으로 섞은 opponent pool을 사용한다.
- training seed와 evaluation seed, opponent snapshot 및 rules config를 분리한다.
- search가 만든 state/action target은 held-out 성능을 개선할 때만 초기 학습에 사용한다.
- frozen evaluation suite에서 통계적으로 유의한 개선이 있을 때만 checkpoint를 승격한다.
- opener, downstack, pressure, defense style cluster 중 하나라도 크게 회귀하면 승격하지 않는다.
- 현재 self-play policy 하나만 상대로 평가하지 않는다.

다음 지표를 보고한다.

- paired match win rate와 신뢰구간
- 불확실성을 포함한 Elo 유사 rating
- opponent pool에 대한 exploitability proxy
- APP, APL, PPS, 생존 시간
- inference latency와 nodes per move
- 환경 sample 수, wall time 및 측정 가능한 경우 energy 사용량

non-learning baseline은 반드시 유지한다.

- random legal bot
- Dellacherie/Thiery 계열 선형 evaluator
- beam/MCTS bot
- license가 허용하는 archived external-bot protocol baseline

## 12. 제한된 연산 자원을 위한 최적화 계획

- Rust bitboard에서 worker별 다수의 독립 match를 batch 처리한다.
- 학습 중 rendering을 수행하지 않는다.
- Python 경계에는 packed observation과 재사용 buffer를 사용한다.
- batched step은 GIL을 해제한다.
- afterstate 열거와 중복 제거를 native code에서 수행한다.
- transposition cache key는 `(board, queue, hold, versus_context, rules_hash)`로 구성한다.
- simulation, feature extraction, bridge, inference와 optimizer를 분리해 profile한 뒤 실제 병목만 최적화한다.
- CPU에서 선형·소형 모델부터 시작하고 end-to-end throughput이 실제로 향상될 때만 GPU를 사용한다.
- FP32 결정론적 기준선을 만든 뒤 mixed precision, compile 및 asynchronous rollout을 검토한다.
- small/medium/full budget을 사전 선언하고 successive halving으로 검증되지 않은 모델의 비용을 제한한다.

## 13. 단계별 실행 계획

### Phase 0 — 근거 확보 및 specification 고정

**수행 내용**

- CPU core, RAM, GPU/VRAM, OS와 container 제한을 기록한다.
- 목표 inference time과 학습 wall-time budget을 수치로 정한다.
- 사용자가 소유한 TETR.IO custom/replay fixture와 export option을 확보한다.
- `configs/rules/`에 값, 단위, version, confidence 및 fixture를 가진 source ledger를 만든다.
- conformance matrix를 test ID와 예상 관찰 결과로 변환한다.

**통과 조건:** 필요한 TETRA LEAGUE 규칙 상수를 임의로 추정하지 않았으며, 모든 unknown에 확인 실험이 연결되어 있어야 한다.

### Phase 1 — 저장소와 결정론적 엔진 골격

**수행 내용**

- Rust workspace, Python package, container workflow, CI, license와 표준 폴더를 만든다.
- bitboard field, piece geometry, seedable RNG/7-bag, spawn/hold/queue, SRS+ 및 reachable placement 열거를 구현한다.
- unit, property, golden, fuzz test와 step/collision/clear/afterstate benchmark를 작성한다.

**현재 진척:** Rust workspace와 container workflow, 10×40 bitboard, piece geometry, MINSTD 기반 generic 7-bag, configurable spawn/hold/queue, 공개 자료 기반 SRS+/180 candidate, geometric reachable-lock BFS가 구현되었다. 이어서 유리수 frame gravity, lock delay/reset cap, client-derived gravity schedule과 근거를 가진 `rules-tetrio` profile을 구현했다. 필수 timing literal은 모두 `OBSERVED`로 채워 실행 가능하지만 replay conformance blockers는 유지한다. 현재 전체 38개 unit test가 통과한다. spawn/RNG/kick/timing literal은 reference fixture 전까지 target conformance로 확정하지 않는다.

**통과 조건:** debug와 release의 replay/state hash가 같고 C1 geometry/RNG/movement fixture가 통과해야 한다.

### Phase 2 — Timing 및 solo profile

**수행 내용**

- frame 입력 순서, DAS/ARR/SDF, IRS/IHS, gravity, lock/reset, ARE, line-clear delay와 top-out을 구현한다.
- line/spin/perfect-clear 및 40 LINES, BLITZ, ZEN/custom 점수 profile을 구현한다.
- 엔진 state 밖에 진단용 최소 CLI/replay viewer를 만든다.

**현재 진척:** float 없는 유리수 gravity accumulator, client option 기반 초기 `0.02G`와 120초 뒤 초당 `0.0035G` 증가 및 20G cap 계산, hard drop 즉시 lock, 30-frame lock과 15회 move/rotation reset이 구현되었다. ordered edge와 held state를 DAS/ARR/DCD/sonic-drop action으로 바꾸는 generic normalizer도 추가했다. TL의 `room_handling=false` 때문에 개인 DAS/ARR/DCD/SDF는 고정 mode 값에서 분리된다. IRS/IHS, exact same-frame stage order, spin/top-out과 replay 연결은 아직 남아 있다.

**통과 조건:** timing 경계 fixture와 solo 전체 replay에서 설명되지 않은 차이가 0개여야 한다.

### Phase 3 — TETRA LEAGUE Season 2 versus

**수행 내용**

- base attack, combo, B2B Charging/Surge, opener 상쇄, garbage-clear bonus를 구현한다.
- garbage packet transit/cancel/cap/messiness/insertion과 round-terminal rule을 구현한다.
- 결정론적 2인 scheduling과 명시적인 latency model을 추가한다.
- 최소화한 사례에 이어 10,000개 이상의 seed 기반 무작위 differential case를 실행한다.

**통과 조건:** 선언된 corpus에 대해 C2~C5가 통과하고 남은 `UNCONFIRMED` 항목과 coverage가 공개되어야 한다.

### Phase 4 — Bot arena 및 강한 baseline

**수행 내용**

- bot protocol adapter, 시간·node budget, 재현 가능한 tournament와 opponent snapshot을 구현한다.
- random, 선형 feature, beam/MCTS 및 허용되는 external protocol baseline을 추가한다.
- strength, latency, throughput 및 style cluster 기준선을 만든다.

**통과 조건:** 아직 학습 시스템을 사용하지 않고도 신뢰구간이 있는 반복 가능한 rating을 생성할 수 있어야 한다.

### Phase 4.5 — 휴리스틱 기록과 모방학습 bootstrap

**수행 내용**

- mechanics conformance를 통과한 arena에서 여러 linear/search teacher와 상대 pool의 기록을 생성한다.
- 선택 행동뿐 아니라 모든 legal afterstate의 score/rank, teacher margin·style·budget, rules/engine hash, seed와 결과를 shard에 저장한다.
- chosen-action BC, full-score/rank distillation과 value initialization을 비교한다.
- learner가 방문한 상태에 teacher를 다시 질의하는 dataset aggregation을 수행한다.
- `100k` smoke, `1M` pilot, `10M` medium decision 순서로 strength-per-byte/second를 확인한 뒤 larger dataset을 승인한다.

**통과 조건:** imitation checkpoint가 같은 latency budget의 random initialization과 chosen-only BC보다 held-out closed-loop 대국에서 강하고 illegal action이 0개여야 한다. offline accuracy만으로는 통과하지 못한다.

### Phase 5 — RL environment 및 보상 검증

**수행 내용**

- GIL-free batched afterstate environment와 observation/action schema를 제공한다.
- terminal zero-sum reward와 potential shaping framework를 구현한다.
- reduced-game exact policy-invariance test 및 feature별 bound/symmetry test를 실행한다.
- linear, MLP, hybrid spatial과 algorithm 비교를 고정 예산으로 사전 등록한다.

**통과 조건:** reward theorem의 가정과 test가 통과하고 모델이 hidden information이나 illegal action을 사용할 수 없어야 한다.

### Phase 6 — 학습 단계

**수행 내용**

- 선형 scorer와 소형 MLP를 차례로 학습한다.
- 앞선 결과가 정당화할 때만 hybrid encoder를 학습한다.
- PPO 후보를 noisy cross-entropy/ES 및 승인된 imitation 초기화와 비교한다.
- historical opponent pool과 exploit policy를 추가한다.
- reward feature ablation과 paired 통계 분석을 수행한다.

**통과 조건:** 고정된 wall-time 및 inference budget에서 held-out 상대에 대한 성능이 가장 높은 구조를 선택하고, 남긴 모든 shaping feature에 근거가 있어야 한다.

### Phase 7 — 성능 엔지니어링

**수행 내용**

- native simulation, afterstate enumeration, bridge, inference 및 optimizer를 profile한다.
- 측정 결과에 따라 SIMD/bit trick, transposition cache, batching, buffer reuse, async rollout과 안전한 reduced precision을 적용한다.
- 각 최적화가 golden replay와 허용 오차 내 모델 평가 결과를 바꾸지 않는지 확인한다.

**통과 조건:** 기록된 제한 hardware에서 목표 steps/s와 move latency를 달성하고 conformance 회귀가 없어야 한다.

### Phase 8 — 최종 검증 및 인수인계

**수행 내용**

- 전체 conformance corpus, fuzz/property suite, release build, tournament, ablation 및 재현성 rerun을 실행한다.
- 모든 `PROJECT_Explain_*` 문서에 최종 상수, 모델, reward weight, benchmark와 한계를 반영한다.
- local play, arena, training 명령과 checkpoint/model card를 제공한다.

**통과 조건:** 다른 개발자가 container만으로 engine test, 소규모 학습·평가 및 공개 tournament replay를 재현할 수 있어야 한다.

## 14. 핵심 위험과 대응

- **Upstream 불투명성 및 drift:** version과 fixture를 고정하고 최신 동작을 자동 반영하지 않는다.
- **Replay format/API 변경:** user-owned export를 내부 normalized schema로 변환한다.
- **거짓 동등성 주장:** fixture coverage와 confidence를 공개하고 근거 없는 전체 동등성 표현을 피한다.
- **Reward hacking:** terminal objective, potential-only shaping, reduced-game exact test와 opponent-pool 평가를 사용한다.
- **Self-play 순환 상성:** historical/exploit opponent 및 held-out style cluster를 유지한다.
- **연산 자원 고갈:** native batching, small-model-first, successive halving 및 hard wall-time budget을 사용한다.
- **법적·공정 이용 문제:** local-only arena, 독자 asset과 naming, license 검토 및 live-service automation 금지를 지킨다.

## 15. 즉시 수행할 작업

1. 대표 TETR.IO replay/config export를 확보해 현재 `OBSERVED` timing literal과 exact frame order를 fixture로 승격한다.
2. generic normalizer에 player/replay handling config serialization, IRS/IHS와 target stage-order adapter를 추가한다.
3. timing/handling state를 lock/line-clear/replay state transition과 연결하고 최초 divergence를 보고하는 fixture manifest를 만든다.
4. CPU core, RAM, GPU/VRAM을 조사해 engine throughput과 bot inference budget을 수치로 선언한다.
5. spin/attack/garbage/round terminal conformance를 통과한 뒤 휴리스틱 기록 생성을 시작한다.

현재 가장 중요한 작업은 코드를 빠르게 작성하는 것이 아니라 **무엇을 동일하게 만들어야 하는지 증거로 고정하는 것**이다. 규칙이 틀린 빠른 engine이나 잘못된 보상을 최적화한 강한 모델은 프로젝트 목표를 달성하지 못한다.

## 16. 주요 연구 및 구현 근거

### TETR.IO 및 규칙 자료

- [TETR.IO 공식 patch note](https://tetr.io/about/patchnotes/): Season 2, garbage, rotation 및 timing 변경의 우선 근거
- [TETR.IO FAQ mechanics](https://github.com/tetrio/faq/blob/main/mechanics.html): DAS/ARR/DCD/SDF 등 공개 handling 설명
- [TETR.IO API 문서](https://tetr.io/about/api/): 공개 서비스 API 범위 확인용이며 engine specification으로 취급하지 않음
- [TETR.IO 이용 약관](https://tetr.io/about/terms/): online service와 local project의 경계 확인
- [TETR.IO Wiki Mechanics](https://tetrio.wiki.gg/wiki/Mechanics): SRS+/SRS-X, combo/B2B, spin과 공격 설명의 2차 자료
- [TETRA LEAGUE Wiki](https://tetrio.wiki.gg/wiki/TETRA_LEAGUE): Season 경계 및 Season 2 요약
- [TetrisWiki의 TETR.IO 문서](https://tetris.wiki/Tetr.io): QUICK PLAY 등 mode 차이 교차 확인

### 구현, protocol 및 경험 기록

- [Cold Clear](https://github.com/MinusKelvin/cold-clear): Rust 기반 modern-versus bot의 architecture 기준선
- [Tetris Bot Protocol](https://github.com/tetris-bot-protocol/tbp-spec): 로컬 frontend와 bot 사이의 공통 interface
- [TETR.IO bot protocol notes](https://github.com/lemoncove/tetrio-bot-docs): room option과 piece RNG 관찰값. fixture 확인 전까지 `OBSERVED`
- [tetris-analyzes](https://github.com/EdamAme-x/tetris-analyzes): current-client table 재현 추출과 freshness-check에 사용. 두 asset snapshot의 TL option 31개가 동일했지만 reference replay가 없으므로 `OBSERVED` 근거이며 authority로 승격하지 않음
- [Fan attack calculator](https://github.com/skysomorphic/tetrio-attack-calculator): 과거 공식과 불확실성을 보여 주는 반례 자료
- [Reddit garbage discussion, 2024-10-30](https://www.reddit.com/r/Tetris/comments/1gfo4ss/how_does_tetrio_garbage_work/): transit/cancel/passthrough edge case 발견용
- [Reddit Jstris/TETR.IO comparison, 2021-09-25](https://www.reddit.com/r/Tetris/comments/pv32r6/are_jstris_and_tetrio_different/): version drift 사례
- [Reddit Tetris AI experience](https://www.reddit.com/r/Tetris/comments/udnowg/i_pit_my_old_tetris_bot_against_my_new_tetris_bot/): heuristic pruning과 lookahead 가설 자료

GitHub와 Reddit 자료는 구현 아이디어와 edge case를 찾는 데 사용하지만, 공식 규칙의 최종 근거로 사용하지 않는다.

### 학습 및 보상 연구

- [Szita & Lorincz, 2006](https://www.cs.utexas.edu/~shivaram/readings/b2hd-SzitaLorincz2006.html): compact feature와 noisy cross-entropy baseline
- [Thiery & Scherrer, 2009](https://journals.sagepub.com/doi/pdf/10.3233/ICG-2009-32104): BCTS 계열 feature 및 cross-entropy 개선
- [Scherrer et al., 2015](https://jmlr.org/papers/v16/scherrer15a.html): approximate modified policy iteration과 sample efficiency
- [Algorta & Simsek, 2019](https://arxiv.org/abs/1905.01652): Tetris machine learning 연구의 역사 및 open challenge
- [Chen et al., 2026](https://arxiv.org/abs/2603.26765): bitboard, afterstate와 buffer-optimized PPO. 최신 preprint이므로 독립 재현 필요
- [Ng, Harada & Russell, 1999](https://ai.stanford.edu/~ang/papers/shaping-icml99.pdf): potential-based reward shaping의 policy-invariance 근거
- [Lu, Schwartz & Givigi, 2014](https://arxiv.org/abs/1401.3907): stochastic game에서 potential shaping의 Nash-equilibrium invariance 근거
- [Devlin & Kudenko, 2011](https://www.ifaamas.org/Proceedings/aamas2011/papers/D1_G45.pdf): multi-agent potential shaping의 조건과 주의점
- [OpenSpiel, 2019](https://arxiv.org/abs/1908.09453): multi-agent self-play와 평가 framework 참고
- [PSRO, 2017](https://mlanctot.info/files/papers/nips17-psro.pdf): nonstationarity 및 순환 policy를 다루는 population training 참고
- [Stanford CS224R multi-agent Tetris project, 2025](https://cs224r.stanford.edu/projects/pdfs/224R_Paper__1_.pdf): custom environment의 탐색 가설이며 model 선택의 확정 근거가 아님
- [Zhang, Cai & Nebel, 2010](https://www.eurosis.org/cms/files/proceedings_full/GAMEON2010.deel1_2.11.10.rdo.pdf): Tetris placement imitation을 classification으로 구성한 직접 선행 사례
- [DAgger, 2011](https://proceedings.mlr.press/v15/ross11a.html): learner가 유발한 state distribution에서 label을 다시 모으는 근거
- [DQfD, AAAI 2018](https://ojs.aaai.org/index.php/AAAI/article/view/11757): demonstration supervised loss와 TD 학습 결합의 근거
- [Beliaev et al., ICML 2022](https://proceedings.mlr.press/v162/beliaev22a.html): 여러 teacher의 상태별 expertise 차이를 다루는 근거
- [Expert Iteration](https://arxiv.org/abs/1705.08439): search teacher와 apprentice 반복 개선 구조

연구 해석 시 다음 한계를 지킨다.

- Season 1 공격표를 최신 Season 2 규칙 근거로 직접 사용하지 않는다.
- solo Tetris 점수만으로 1 대 1 성능을 주장하지 않는다.
- 2026 bitboard 연구는 다른 board/generator 조건의 결과이므로 실험 후보를 정당화할 뿐 최종 model을 결정하지 않는다.

세부 mechanics 조사, RL 조사와 주장별 출처 제한은 각각 `research/TETRIO_MECHANICS_RESEARCH_KO.md`, `research/RL_RESEARCH_KO.md`, `research/SOURCE_LEDGER.md`에 정리한다.

## 17. 완료 정의

각 phase는 다음 조건을 모두 만족해야 완료로 처리한다.

- acceptance gate가 실제로 통과했다.
- format, lint, unit/property test, type check와 release build를 가능한 범위에서 실행했다.
- 해당 profile의 conformance replay suite와 benchmark를 실행했다.
- 오류와 경고를 해결했거나 범위 밖으로 합의한 사유를 기록했다.
- 결과와 영향, 남은 제한을 `.agent/CONTINUITY.md`와 관련 `PROJECT_Explain_*` 문서에 반영했다.

규칙 정확성 gate가 강화학습보다 먼저다. 결정론 및 conformance 검증을 통과하지 않은 profile에서는 모델 학습을 시작하지 않는다.
