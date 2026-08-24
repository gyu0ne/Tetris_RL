# 전투 스케줄러와 Current Client Mechanics 설계 설명

상태: 선언한 학습 관련 mechanics의 `OBSERVED` 실행 구현 완료, 외부 differential 인증 미완료
기준 시각: `2026-08-24T20:29:16+09:00`

## 1. 기준과 범위

기준 client는 `TETR.IO BETA 1.7.8 / TETRA LEAGUE Season 2`다. 직접 대조한 bundle은 다음과 같다.

- URL: `https://tetr.io/js/tetrio.js?hv=63ab5c7c7.efa161fa8f91.20260810T191705`
- SHA-256: `AAB6D586AAAEF57F84553CBD60237604832BE420FA2B27773B6E697F66B84D66`

이 bundle에서 추출한 상수와 제어 흐름은 `OBSERVED`다. 계정, matchmaking, rating, UI, 음향, 40 LINES/BLITZ 점수는 구현 범위가 아니다. legal afterstate, board transition, attack/garbage, RNG 분포와 round terminal reward를 바꾸는 mechanics만 필수 범위다.

## 2. Piece와 frame mechanics

- piece와 garbage는 같은 초기 seed에서 시작하는 서로 분리된 Park–Miller MINSTD stream을 쓴다.
- bag shuffle 입력 순서는 `ZLOSIJT`다.
- board occupancy는 fractional `y`의 ceiling을 사용한다.
- spawn fall phase는 `0.96`, fallback kick 뒤 phase는 `0.1`이다.
- direct 회전과 첫 fallback kick은 upstream 번호 `0`, 이후 fallback은 `1..3`이다.
- T-spin candidate에서 upstream kick 번호가 `3`이면 Full로 승격한다.
- 자동 fall이 막힌 frame에 lock counter가 증가하며 `locking > locktime`에서 lock한다.
- 성공 move/rotation이 reset cap에 도달한 입력은 counter를 증가시키지만 lock timer를 초기화하지 않는다.

부동소수점 위치를 그대로 장기 누적하지 않고 client의 1e-6 quantization을 정수 microcell phase로 표현한다. attack combo의 upstream `Math.log1p`와 garbage multiplier의 frame별 `+= 0.008/60`처럼 IEEE-754 누적 오차가 실제 floor 결과를 바꾸는 경로만 guarded `f64`로 재현한다.

## 3. 한 player의 lock transaction

```text
ordered frame actions
  -> hold 전후 piece action 분리
  -> movement / rotation / fall / lock
  -> clear, spin, perfect-clear, garbage provenance
  -> attack packet 계산
  -> 다음 piece 또는 IHS replacement spawn
  -> post-clear Clutch 이동
  -> block-out 확정
```

같은 frame의 action은 입력 순서를 보존한다. 특히 rotate→hold와 hold→rotate는 서로 다른 active piece에 작용하므로 하나의 boolean hold request로 합치지 않는다. raw 브라우저 event의 0.1-subframe timestamp는 replay adapter가 필요할 때만 처리한다.

## 4. 두 player 동시 스케줄링

`BattleSession`은 한쪽 처리 결과가 같은 tick의 상대 행동을 선점하지 않도록 다음 순서를 사용한다.

1. 양쪽 frame action과 lock 결과를 각각 계산한다.
2. 각 lock에서 clear/attack packet과 기존 incoming 상쇄를 확정한다.
3. 같은 tick에 남은 outgoing packet을 상대 queue로 동시에 전달한다.
4. clear가 없는 player에 대해 ready garbage를 cap 안에서 삽입한다.
5. Clutch와 garbage/block-out을 반영한 뒤 양쪽 terminal을 함께 판정한다.

따라서 same-tick 양쪽 사망은 처리 순서에 따른 승패가 아니라 무승부다. 한쪽만 terminal이면 다른 쪽이 승리한다.

## 5. Garbage pipeline

공격 packet은 attack-first, opener-budget-second 순서로 incoming을 상쇄한다. zero passthrough이므로 남은 공격만 상대에게 전달한다. 수신측 garbage generator는 change-on-attack 및 `messiness_change=1`, `messiness_inner=0`에 따라 packet 내부에서 같은 hole을 공유한다. packet이 삽입이나 상쇄로 소진될 때 change 검사와 다음-hole 표본을 소비하므로 완전 상쇄도 이후 hole sequence를 진행시킨다.

수신 queue는 20-frame transit, combo blocking, placement당 cap 8을 적용한다. 180초 margin 이후 multiplier는 client와 같은 frame-end IEEE-754 `+= 0.008/60`으로 증가한다. 이를 exact rational 곱으로 바꾸면 정수 경계에서 floor 결과가 달라진다. board의 occupancy와 garbage provenance를 함께 이동·압축해 difficult garbage clear `+1`을 실제 지워진 cell에서 계산한다. 40-row ceiling을 밀어낸 lethal garbage는 terminal에 반영한다.

## 6. 학습 action과 raw input의 경계

주 학습 action은 engine이 열거한 reachable locked-afterstate다. 정책은 OS key repeat나 사용자의 DAS/ARR 설정을 직접 선택하지 않는다. 그러므로 raw 0.1-subframe 입력 재생의 완전한 복제는 주 학습 상태 전이의 완료 조건이 아니다. frame-aligned normalizer는 solo sandbox와 replay 검증 편의 계층이며, 개인 handling을 TL 고정 mechanics라고 주장하지 않는다.

## 7. 검증과 남은 주장 한계

구현 검증은 workspace 110개 unit/integration test로 다음을 포함한다.

- MINSTD/bag 결정론, rotation/kick, fractional fall과 lock 경계
- same-frame hold order, IRS/IHS, spin/perfect clear, Clutch/top-out
- attack/B2B/Surge, opener/cancellation, hole RNG와 margin
- garbage insertion/overflow, 양쪽 동시 lock·attack·terminal

이 test는 로컬 명세의 구현 일관성을 입증한다. TETR.IO가 내보낸 동일 seed·동일 입력의 board checkpoint corpus가 아직 없으므로 C1~C5 외부 differential 통과나 공식 conformance를 주장하지 않는다. 해당 fixture가 확보되면 최초 divergence를 반환하는 `replay-conformance`로 `OBSERVED`를 `CONFIRMED`로 승격한다.
