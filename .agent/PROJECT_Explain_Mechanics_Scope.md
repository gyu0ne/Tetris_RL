# Mechanics 동등성 범위 결정

상태: 설계 완료

결정 시각: `2026-08-24T15:05:44+09:00`

## 결정

이 프로젝트에서 “TETR.IO와 정확히 같음”은 게임 서비스 전체가 아니라 **학습할 수 있는 전략, legal action, 상태 전이 또는 terminal reward에 영향을 주는 mechanics의 관찰적 동등성**을 뜻한다.

동일한 rules profile, seed, 초기 상태와 frame 입력을 사용했을 때 covered fixture에서 board, piece, timer, clear/spin, attack, garbage와 round 결과가 같아야 한다. 내부 구현 언어나 자료구조가 같을 필요는 없다.

## 포함

- board, pieces, RNG, spawn, next, hold
- rotation/kick, movement, handling과 reachability
- gravity, ARE, lock delay/reset과 frame ordering
- clear, spin/mini/full, perfect clear
- attack, combo, B2B Charging, Surge, opener bonus
- garbage 생성·packet·cancel·cap·activation·messiness
- top-out, simultaneous death, Clutch Clear와 round terminal

round terminal은 강화학습의 `+1/-1/0` 보상을 결정하므로 필수다. 승패 조건은 terminal 판정에 필요한 만큼만 포함하며, 누적 점수 체계는 포함하지 않는다.

## 제외

- account/authentication, rating/TR, matchmaking, lobby
- UI, animation, skin, sound/music, chat, social 기능
- pixel-perfect board rendering, block skin/color, transition animation and layout parity
- shop, achievement, profile, moderation, anti-cheat
- official server/network infrastructure의 비공개 구현
- 학습과 무관한 match 사이 progression
- 40 LINES, BLITZ, ZEN/custom의 점수·목표·기록 체계

네트워크 지연은 local arena parameter로 연구할 수 있으나 TETR.IO 서버 동작의 복제라고 표시하지 않는다. 제외된 기능이 관측·행동·보상에 영향을 준다는 증거가 생기면 영향 범위만 다시 포함한다.

## 판단 시험

어떤 기능 `X`를 구현할지 애매하면 다음 질문을 순서대로 적용한다.

1. `X`가 legal action 또는 실제 handling에서의 reachability를 바꾸는가?
2. `X`가 다음 state, 공개 observation 또는 RNG 분포를 바꾸는가?
3. `X`가 attack/garbage timing이나 round terminal을 바꾸는가?
4. `X`가 같은 mechanics에서 학습·평가 편의만 제공하는가?

1~3 중 하나가 참이면 conformance 범위다. 4만 참이면 tooling/arena 기능이며 TETR.IO 동등성 주장에서는 제외한다. 모두 거짓이면 구현하지 않는 것을 기본으로 한다.

## 현재 profile 보정

- 잠정 target은 `TETR.IO BETA 1.7.8 / TETRA LEAGUE Season 2`다.
- 현재 multiplayer 기본은 `All-Mini+`다. Season 2 시작 당시의 `All-Mini`를 현재 규칙으로 사용하지 않는다.
- TL timing·garbage·top-out literal과 제어 흐름은 current client에서 직접 추출한 `OBSERVED` 실행값이다. 외부 기준 board checkpoint를 통과하기 전에는 `CONFIRMED`가 아니다.
- Quick Play는 mechanics가 다르므로 별도 profile이며 core 1 대 1 학습 범위에 포함하지 않는다.

## 설계 영향

- `engine-core`는 deterministic common mechanics만 담당한다.
- `rules-tetrio`는 versioned literal과 실행 순서를 보유한다.
- `versus`는 attack, garbage와 round terminal을 구현하지만 rating/matchmaking을 구현하지 않는다.
- `arena`는 latency/time/node budget 같은 실험 변수를 명시하며 upstream conformance와 구분한다.
- `manual-playground`는 실제 Rust state를 단순 표시하는 진단 도구다. 화면 모양은 동등성 근거가 아니며 JS에서 mechanics를 다시 구현하지 않는다.
- heuristic/모방학습의 exploratory 단계는 deterministic core test와 수동 mechanics smoke 뒤 시작할 수 있다. formal conformance는 최종 동등성 주장과 release benchmark의 gate다.

상세 근거와 미확정 항목은 `research/TETRIO_MECHANICS_RESEARCH_KO.md` 및 `research/SOURCE_LEDGER.md`를 따른다.
