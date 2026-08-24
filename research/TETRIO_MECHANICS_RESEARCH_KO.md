# TETR.IO mechanics 조사 정리

조사 기준 시각: `2026-08-24T15:05:44+09:00`

잠정 기준: `TETR.IO BETA 1.7.8 / TETRA LEAGUE Season 2`

## 1. 동등성 범위 결정

이 프로젝트의 “정확히 같음”은 제품 전체가 아니라 다음 관찰적 mechanics 동등성을 뜻한다.

> 동일한 rules profile, seed, 초기 상태와 frame별 입력을 주었을 때, 기준 fixture가 포괄하는 모든 상황에서 두 엔진이 동일한 상태 전이, 공격·garbage event와 round 종료 결과를 생성한다.

### 반드시 같은 대상

| 영역 | 포함 이유 |
|---|---|
| 보드 크기·hidden row·piece geometry·spawn | 가능한 배치와 top-out이 달라짐 |
| piece randomizer·seed·next·hold | 관측과 장기 계획이 달라짐 |
| SRS+ 회전·wall kick·180도 회전 | 도달 가능한 afterstate가 달라짐 |
| DAS·ARR·SDF·DCD·IRS·IHS와 입력 우선순위 | 같은 시간 예산에서 도달 가능성이 달라짐 |
| gravity·ARE·line-clear ARE·lock delay/reset | 행동 가능 시간과 생존 전략이 달라짐 |
| line clear·spin·mini/full·perfect clear | 공격량과 B2B 상태가 달라짐 |
| combo·B2B Charging·Surge·opener 보정 | 1 대 1 공격 전략이 달라짐 |
| garbage packet·cancel·blocking·cap·messiness·activation | 방어·downstack·spike timing이 달라짐 |
| block-out·lock-out·동시 사망·clutch clear·round 승패 | terminal reward가 달라짐 |

### 기본적으로 제외하는 대상

- 계정, 인증, 랭크·TR/Glicko 표시, 매치메이킹, lobby 탐색
- UI, skin, animation, 음향, 음악, 채팅, replay viewer의 시각 표현
- 상점, 외형, 업적, 통계 페이지, anti-cheat, moderation
- 공식 서버의 비공개 네트워크·인프라 구현
- match 사이의 progression이나 rating 변화

단, 제외 대상이 관측·행동·보상·episode 종료를 바꾼다는 증거가 생기면 해당 부분만 mechanics 범위로 승격한다. 네트워크 지연은 TETR.IO의 비공개 구현을 복제한다고 주장하지 않고 local arena의 명시적 실험 변수로만 둔다.

## 2. 버전 기준과 변경 이력

공식 patch note를 2026-08-24에 확인한 결과 최신 표시 버전은 `BETA 1.7.8`이며 배포일은 `2026-04-01`이다. 기준 버전은 자동으로 최신화하지 않고 rules hash와 함께 고정한다.

현재 설계에 직접 영향을 주는 공식 변경은 다음과 같다.

| 버전 | 공식 기록에서 확인한 변경 | 상태 |
|---|---|---|
| BETA 1.2.0, 2024-08-16 | Season 2에서 종전 B2B Chaining을 B2B Charging/Surge로 교체 | `CONFIRMED` |
| BETA 1.1.2 | Surge charge 시작점을 B2Bx4로 복원 | `CONFIRMED` |
| BETA 1.2.0 | 첫 14 pieces 조건의 opener cancellation 보정과 All Clear 5 공격 규칙 확정 | `CONFIRMED` |
| BETA 1.3.0, 2024-09-22 | garbage를 지우는 Quad/Spin 공격에 multiplier의 영향을 받지 않는 flat +1 special bonus | `CONFIRMED` |
| BETA 1.5.0, 2025-01-18 | 모든 multiplayer와 ZEN의 기본을 All-Mini+로 변경 | `CONFIRMED` |
| BETA 1.5.0 | 3-corner를 충족하지 못하지만 immobile인 T-spin을 Mini로 판정 | `CONFIRMED` |
| BETA 1.5.0 | 즉시 block-out될 line clear에서 다음 piece를 위로 밀어 생존 기회를 주는 reworked Clutch Clear | `CONFIRMED` |
| ALPHA 6.1.2, 2021-10-18 | TETRA LEAGUE와 기본 설정에서 passthrough 비활성화 | `CONFIRMED` |

따라서 기존 계획에 적힌 단순 `All-Mini`는 역사적 Season 2 시작 규칙이며 현재 기준 profile은 `All-Mini+`로 수정해야 한다.

## 3. 현재 mechanics 지식

### 3.1 보드·piece·회전

- 기본 logical playfield는 10×40이고 next queue 표시는 5개이며 hold와 hard drop을 사용한다. `OBSERVED`
- 기본 회전 체계는 SRS+다. SRS-X는 선택 가능한 별도 체계이므로 TL 기본 profile에 섞지 않는다. `OBSERVED`
- SRS+는 guideline SRS에 I-piece의 y축 대칭 kick과 TETR.IO의 180도 kick table을 더한다. 정확한 ordered kick table literal은 current-client fixture로 고정해야 한다. `OBSERVED`
- spin은 마지막 유효 동작, immobility, T-piece corner와 kick 결과의 조합에 의존한다. 정확한 mini/full edge case를 표로 만들고 각 회전 상태·kick index에 대한 fixture를 확보해야 한다. `OBSERVED`

### 3.2 입력·시간

- 2026-05-04와 2026-08-10 public client asset을 같은 extractor로 재현 추출했으며 TL option 31개 중 변경된 field는 0개였다. snapshot hash와 extractor revision은 `configs/rules/tetrio-beta-1_7_8-tl-s2.observed.toml`에 고정했다.
- TL 시작 gravity는 `0.02G = 1/50 cell/frame`, gravity margin은 7200 frames, 이후 증가는 초당 `0.0035G = 7/2000G`, cap은 20G로 관찰됐다. ARE와 line-clear ARE는 모두 0이다. `OBSERVED`
- lock delay는 30 frames이고 reset cap은 15회다. Wiki의 기본 500 ms 및 이동·회전 reset 설명과 60 Hz에서 일치한다. `OBSERVED`
- TL의 `room_handling`은 false다. 함께 들어 있는 ARR 2/DAS 10/SDF 6은 강제되지 않는 inactive room fallback이며 실제 DAS/ARR/DCD/SDF는 player config에 의존한다. `OBSERVED`
- ARR 0은 수평 이동을 즉시 벽까지 적용하고, DAS는 ARR 반복 전의 hold 시간이다. `OBSERVED`
- DCD는 회전과 spawn 때 이미 충전된 DAS를 설정 frame만큼 멈춘다. `OBSERVED`
- SDF infinity는 충돌 직전까지 즉시 내리는 sonic drop이며 hard drop처럼 즉시 lock하는 동작과 구별한다. `OBSERVED`
- IRS/IHS의 buffer 시점, 같은 frame 입력 충돌, spawn→gravity→input→lock의 정확한 순서는 differential fixture가 필요하다. `UNCONFIRMED`

정책이 배치만 선택하더라도 move generator가 실제 handling/time budget에서 도달 불가능한 배치를 허용하면 학습 문제가 달라진다. 따라서 “기하학적으로 가능한 배치”와 “profile상 도달 가능한 입력 경로”를 분리해 검증한다.

### 3.3 line·spin·공격

- combo multiplier는 base attack이 양수일 때 `base × (1 + 0.25 × combo)`로 설명되어 있다. base가 0이고 combo가 2 이상이면 `ln(1 + 1.25 × combo)` 계열을 사용한다. `OBSERVED`
- TL/일반 multiplayer는 이 결과에 downward/floor rounding을 적용하고 Quick Play의 stochastic rounding과 구별된다. `OBSERVED`
- B2B를 유지하는 difficult clear는 공격마다 +1을 더하고 charge를 쌓는다. B2Bx4에서 Surge가 4로 시작하고 B2B count에 따라 제한 없이 성장한다. `OBSERVED`, 시작점은 patch note로 `CONFIRMED`
- B2B를 끊으면 저장한 Surge를 3 packet으로 방출하며 3으로 나누어떨어지지 않는 remainder는 앞 packet부터 배분되는 것으로 문서화되어 있다. `OBSERVED`
- opener phase는 첫 14 pieces 동안 적용되고, 해당 round에서 이미 보낸 line보다 pending garbage가 많을 때 cancellation power가 두 배가 된다. `OBSERVED`, 도입 사실은 `CONFIRMED`
- garbage를 포함한 line을 지우는 Quad와 Spin 계열은 flat +1 special bonus를 받는다. multiplier에 의해 확대되지 않는다. `CONFIRMED`
- All Clear의 공격량은 5다. `CONFIRMED`
- 정확한 base attack table, combo index 시작점, rounding 적용 순서, bonus와 B2B/Surge 결합 순서는 current fixture와 client-derived snapshot으로 재검증한다. `UNCONFIRMED`

### 3.4 All-Mini+

현재 기본은 다음 두 판정 경로를 구분한다.

- non-T piece: 회전 뒤 immobile이면 Mini spin 후보로 판정한다. `OBSERVED`
- T piece: 기존 corner 규칙으로 full/mini를 판정하되, 3-corner가 아니더라도 immobile이면 Mini가 될 수 있다. 후자는 BETA 1.5.0 변경으로 `CONFIRMED`다.

구현 시 “spin 이름을 표시하는 UI”가 아니라 공격량·B2B·special bonus에 연결되는 판정 event가 동등성 대상이다.

### 3.5 garbage와 round 종료

- TL은 passthrough를 비활성화한 계열이다. 다만 동일 frame 양측 공격의 cancel/block 순서와 garbage activation timing은 fixture로 고정한다.
- historical bot 문서에는 `garbagespeed`, `garbagecap`, `garbagecapincrease`, `garbagecapmax`, `garbagemultiplier`, `garbagemargin`, `garbageincrease`, `passthrough`, `clutch` 등의 room field가 기록되어 있다. 이 문서는 2022년 client를 대상으로 하므로 현재 기본값을 제공하는 근거로 사용하지 않는다.
- garbage hole generator와 messiness, packet merge/split, queue cap, cancellation order, multiplier margin은 전략에 직접 영향을 주므로 값을 추측하지 않는다. `UNCONFIRMED`
- Clutch Clear는 BETA 1.5.0 이후 현재 mechanics에 포함한다. top-out 판정과 next-piece upward displacement의 정확한 경계는 fixture가 필요하다.
- round 승패, draw/동시 사망과 terminal state는 학습 보상을 결정하므로 포함한다. 반면 rating 계산과 matchmaking은 제외한다.

## 4. Quick Play와 다른 mode의 분리

Quick Play에는 height multiplier, targeting, mod, garbage activation과 rounding 등 TL 1 대 1과 다른 규칙이 있다. 다음 이유로 core profile에서 제외한다.

1. 관측·행동·보상 구조가 1 대 1과 다르다.
2. 한 ruleset에 섞으면 검증 실패의 원인을 찾기 어렵다.
3. 모델이 실제 TL에 없는 신호에 적응할 수 있다.

추후 필요하면 `tetrio-beta-1_7_8-quickplay-*`처럼 별도 rules hash와 fixture suite로 추가한다.

## 5. 아직 확정하면 안 되는 값

다음은 공개 자료만으로 `BETA 1.7.8 / TL S2`의 exact 실행 의미 또는 conformance를 확정하지 못했다.

- client-derived gravity/ARE/lock literal의 reference replay 승격과 margin 경계의 정확한 적용 frame
- player handling 값, IRS/IHS buffer와 입력 frame/stage order
- garbage speed, activation, cap, cap increase/max
- hole generator, messiness, packet merge/split
- garbage multiplier/increase/margin과 cancel/block 순서
- exact base attack table의 모든 spin/kick/combo edge case
- current piece RNG 구현과 seed normalization
- block-out, lock-out, partial lock-out, simultaneous death의 정확한 우선순위
- round option과 win condition 중 mechanics에 해당하는 값

이 목록은 구현 지연 사유가 아니라 conformance 작업 목록이다. provisional 값을 시험용 profile에 넣을 수는 있지만 `tetrio-*` 이름으로 배포하거나 학습 결과를 공식 동등 환경의 결과라고 보고해서는 안 된다.

## 6. fixture 및 differential 검증 계획

### 6.1 필요한 기준 자료

- 사용자가 소유한 현행 TETR.IO replay
- 동일 match의 room/config export와 client/version 식별자
- input frame, piece sequence, board snapshot, attack/garbage event, KO/round event
- 가능한 경우 current client가 실제 사용한 table의 hash가 포함된 snapshot

### 6.2 최소 fixture 축

1. 모든 piece·회전 상태·wall/floor·ordered kick 성공/실패
2. 0/20G, soft/sonic/hard drop, DAS/ARR/DCD, IRS/IHS 충돌
3. line 종류, T mini/full, non-T mini, immobility·last-action·kick-index edge case
4. combo와 B2B 유지/파괴, B2Bx4 전후, Surge의 remainder 분할
5. opener 14-piece 경계, doubled cancellation, special +1, All Clear
6. garbage packet의 arrival/cancel/cap/activation/hole sequence
7. block-out·lock-out·동시 사망·Clutch Clear·round terminal

### 6.3 판정 기준

- 같은 seed와 input stream에서 매 frame의 board hash, active piece, queue/hold, timers, combo/B2B/Surge와 garbage queue가 같아야 한다.
- 공격 packet과 terminal event는 값뿐 아니라 발생 순서와 frame도 같아야 한다.
- 첫 divergence에 source field와 confidence를 표시한다.
- hand-authored fixture, replay fixture, seeded fuzz/differential suite를 모두 통과해야 한다.
- 공개 fixture가 포괄하지 않은 동작은 동등하다고 주장하지 않고 coverage manifest에 남긴다.

## 7. 구현에 주는 결론

- engine core와 TETR.IO rules profile을 분리한다. mechanics 변경은 data/versioned rules로 교체하고 deterministic core는 안정적으로 유지한다.
- 정책 action은 afterstate placement를 기본으로 하되, reachability layer가 실제 timing/handling profile에 맞는 input path를 증명해야 한다.
- round terminal까지만 최소 match domain에 포함한다. rating/lobby/service 시스템은 만들지 않는다.
- current literals를 확보하기 전에는 규칙을 최적화하거나 대규모 학습을 시작하지 않는다. 잘못된 ruleset에서의 강한 정책은 목표에 대한 진전이 아니다.

## 8. 주요 출처

- [TETR.IO 공식 patch notes](https://tetr.io/about/patchnotes/)
- [TETR.IO FAQ mechanics source](https://github.com/tetrio/faq/blob/main/mechanics.html)
- [TETR.IO Wiki Mechanics](https://tetrio.wiki.gg/wiki/Mechanics)
- [TETR.IO Wiki Spins](https://tetrio.wiki.gg/wiki/Spins)
- [TETR.IO Wiki TETRA LEAGUE](https://tetrio.wiki.gg/wiki/TETRA_LEAGUE)
- [TetrisWiki TETR.IO](https://tetris.wiki/Tetr.io)
- [과거 TETR.IO bot Room Config 문서](https://github.com/lemoncove/tetrio-bot-docs/blob/master/Room_Config.md)
- [과거 TETR.IO bot Piece RNG 문서](https://github.com/lemoncove/tetrio-bot-docs/blob/master/Piece_RNG.md)
- [SRS+ 공개 issue 논의](https://github.com/tetrio/issues/issues/506)

출처별 사용 가능 주장과 제한은 `SOURCE_LEDGER.md`에 정리한다.
