# 1 대 1 공격 상태 전이 설계

상태: major design 및 observed profile 첫 구현 완료

기준 시각: `2026-08-24T18:00:13+09:00`

## 1. 범위 결정

솔로 플레이는 엔진 생존·상태 전이·봇 smoke test용 무점수 sandbox로 사용한다. 40 LINES, BLITZ, ZEN/custom의 점수와 목표는 구현하지 않는다. 1 대 1 승패에 직접 영향을 주는 clear attack, combo, B2B, Surge, Perfect Clear와 garbage-clear special bonus만 versus mechanics로 구현한다.

책임 경계는 다음과 같다.

```text
engine-core lock
  -> ClearEvent(piece, lines, spin, perfect_clear)
  -> versus::resolve_attack(previous_state, context, rules)
  -> next combo/B2B state + ordered attack packets
  -> implemented garbage cancellation/insertion layer
```

`ClearEvent`에는 점수나 공격 상수가 없다. `versus`에는 보드 이동, 렌더링, 솔로 점수나 학습 보상이 없다. TETR.IO별 literal은 `rules-tetrio`와 versioned TOML record가 소유한다.

## 2. 현재 근거 snapshot

독립 extractor `tetris-analyzes` revision `712dc10be43d5a6c54a35b62608ab9f4a2eaa324`를 container에서 current public client asset에 다시 실행했다.

| 항목 | 값 |
|---|---|
| client asset | `63ab5c7c7.efa161fa8f91.20260810T191705` |
| generated firepower cases | 53 |
| firepower snapshot SHA-256 | `b92d2446e42752a8ba86d873696a83cee0d99223d4bdafc1355a22cabbb3206b` |
| confidence | `OBSERVED` |

2026-05 snapshot과 current snapshot의 공격 table/case 결과 차이는 없고 source identity만 바뀌었다. 그래도 독립 extractor와 public client 관찰은 reference replay differential을 대신하지 않으므로 `CONFIRMED`로 승격하지 않는다.

현재 관찰값은 다음과 같다.

- normal clear: `[0, 0, 1, 2, 4]`
- Mini spin: `[0, 0, 1, 2, 4]`
- Full spin: `[0, 2, 4, 6, 10]`
- B2B continuation bonus: `+1`
- B2B Charging threshold: previous B2B가 4를 초과할 때 break 시 `previous - 4` Surge
- Perfect Clear: 별도 `5` attack packet, B2B credit `+1`
- garbage를 포함한 Quad/Spin clear: rounding 뒤 `+1`
- combo: 공격이 양수면 `floor(attack × (1 + 0.25 × (combo - 1)))`; 0이면 logarithmic minimum

## 3. 부동소수점 제거

current client는 combo의 0-base minimum에 logarithm을 사용한다. authoritative state transition에 platform-dependent 부동소수점을 두지 않기 위해, 각 정수 공격량 `k`가 처음 발생하는 combo index를 정수 임계표로 변환했다.

```text
attack k:  1, 2,  3,  4,   5,   6,   7,    8, ...
min index: 2, 6, 16, 43, 118, 322, 877, 2384, ...
```

양수 공격의 multiplier는 분자·분모가 명시된 정수 연산으로 내림한다. 모든 곱셈과 counter 증가는 checked arithmetic을 사용하고 overflow를 silent saturation하지 않는다. 이 변환은 generated combo cases와 14-double sequence `[1,1,1,1,2,2,2,2,3,3,3,3,4,4]`로 검사한다.

## 4. B2B·Surge와 packet 순서

line clear가 없는 placement는 combo를 0으로 만들지만 B2B를 보존한다. difficult clear는 line을 지우는 Quad 또는 Mini/Full spin이며 B2B를 증가시킨다. ordinary line clear는 B2B를 끊는다. Perfect Clear credit은 이 판정 전에 더해져 normal clear라도 chain을 유지할 수 있다.

current client의 B2B break는 normal clear 공격을 계산하기 전에 Surge를 보낸다. Surge 총량 `s`는 다음 세 값으로 분할하고 0 packet은 생략한다.

```text
q = round(s / 3) = (s + 1) / 3  # s는 양의 정수
[q, q, s - 2q]
```

따라서 `s=5`는 `[2,2,1]`, `s=4`는 `[1,1,2]`, `s=1`은 `[1]`로 방출된다. 이는 과거 문서의 “remainder를 앞 packet부터 배분” 설명을 대체한다. 최종 packet 순서는 `Surge 최대 3개 → current clear → Perfect Clear`다. cancellation layer는 이 순서를 재정렬하면 안 된다.

## 5. API와 오류

- `AttackState`: 현재 combo와 B2B count
- `AttackContext`: cleared row에 garbage cell이 포함됐는지 여부
- `AttackRules`: base table, combo rational/thresholds, B2B/Surge, Perfect Clear와 special bonus
- `AttackOutcome`: 다음 상태, 세부 attack 성분과 최대 5개의 fixed-capacity ordered packet
- `AttackError`: 잘못된 profile, 4줄 초과 clear, line 없는 Perfect Clear, counter overflow, packet capacity 위반

후속 garbage pipeline에서 `Board`의 occupancy와 garbage provenance layer가 함께 구현됐다. lock/line compaction이 계산한 `PlacementOutcome::cleared_garbage`는 `AttackContext`로 직접 변환할 수 있으므로 이 boolean을 더 이상 추측하지 않는다. 자세한 queue·상쇄·삽입 경계는 `PROJECT_Explain_Garbage_Pipeline.md`를 따른다.

## 6. 검증과 미완료 범위

현재 test는 base clear/spin table, 14-double combo, 5회 T-spin-double B2B sequence, no-clear combo/B2B 경계, B2B break packet order, `s=5` Surge 분할, separate Perfect Clear와 post-rounding special +1을 검사한다. `rules-tetrio` test는 current fixture ID를 가진 observed profile이 실행되는지 확인한다.

이 문서 작성 뒤 완료된 부분은 incoming ordered queue, packet cancellation conservation, 14-piece opener double-cancel, 20-frame transit gate, combo blocking, 8-line cap과 board provenance다. 아직 완료되지 않은 부분은 다음과 같다.

1. change-on-attack messiness/hole RNG와 packet 소진 시 RNG 소비
2. garbage margin multiplier 증가
3. 결정론적 두 player frame scheduling과 simultaneous terminal
4. reference fixture를 사용한 `OBSERVED`에서 `CONFIRMED` 승격

따라서 이 단계는 TL 공격 상태 전이의 실행 가능한 observed 구현이지, 전체 1 대 1 mechanics conformance 완료가 아니다.
