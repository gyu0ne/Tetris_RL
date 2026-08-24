# 1 대 1 Garbage Queue·상쇄·삽입 설계

상태: major design 및 current TL observed 구현 완료

기준 시각: `2026-08-24T19:17:00+09:00`

## 1. 범위와 책임 경계

이번 단계는 ordered attack packet이 생긴 뒤의 세 전이를 구현한다.

```text
versus::AttackPackets
  -> incoming queue와 packet 단위 상쇄
  -> 상대에게 보낼 잔여 packet
  -> 20-frame transit gate
  -> non-clear placement에서 최대 8줄 board 삽입
```

`versus`는 incoming packet, round 누적 송신량, opener 상쇄와 삽입 cap을 소유한다. `engine-core::Board`는 규칙 상수를 갖지 않고, 점유층과 garbage provenance층을 함께 이동·압축하는 원시 연산만 제공한다. TETR.IO literal과 관찰 근거는 `rules-tetrio`와 versioned TOML이 소유한다.

## 2. Current client 관찰값

공개 client asset `63ab5c7c7.efa161fa8f91.20260810T191705`를 `2026-08-24`에 직접 확인했다. bundle SHA-256은 `aab6d586aaaef57f84553cbd60237604832be420fa2b27773b6e697f66b84d66`이고, 보조 extractor repository revision은 `712dc10be43d5a6c54a35b62608ab9f4a2eaa324`다.

| TL option/control flow | 값 |
|---|---:|
| `garbagespeed` | 20 frames |
| `garbagecap` | 8 lines |
| `garbagecapincrease` | 0 |
| `garbagecapmax` | 40 |
| `garbageblocking` | `combo blocking` |
| `passthrough` | `zero` |
| `openerphase` | 14 pieces |
| default `cancelmultiplier` | 1 |
| default `garbageentry` | `instant` |
| default `garbagequeue` / `garbagephase` | false / 0 |
| default `messiness_change` / `messiness_inner` | 1 / 0 |

이 값은 실행 가능한 `OBSERVED` 근거다. reference replay의 queue/board checkpoint와 대조하기 전에는 전체 conformance를 `CONFIRMED`로 부르지 않는다.

## 3. Packet 단위 상쇄와 opener

current client의 `FightLines(s)`는 ordered packet 하나마다 다음 순서를 반복한다.

1. packet 공격량 `s`를 incoming queue 앞에서부터 상쇄한다.
2. 첫 14 pieces이고 `pending >= round sent total`이면 별도의 `s`만큼 opener cancellation budget을 만든다.
3. opener budget은 공격량 budget이 모두 소모된 뒤에만 사용한다.
4. 남은 공격량만 상대에게 보내고 round sent total에 즉시 더한다.
5. 다음 Surge/current-clear/Perfect-Clear packet은 갱신된 pending과 sent total로 조건을 다시 계산한다.

따라서 opener는 최종 합계에 한 번 곱하는 규칙이 아니다. packet 순서와 중간 sent total이 결과를 바꾼다. hardened packet은 queue 순서를 유지하지만 상쇄 대상에서 건너뛰며, 뒤의 normal packet은 계속 상쇄할 수 있다. `passthrough=zero`이므로 transit이 끝나지 않은 packet도 상쇄 대상이지만 board 삽입 대상은 아니다.

다음 보존식을 각 호출에서 검사할 수 있다.

```text
raw attack = attack-cancelled + outgoing attack
pending before - pending after = attack-cancelled + opener-bonus-cancelled
sent after = sent before + outgoing attack
```

모든 counter는 정수 checked arithmetic을 사용한다.

## 4. Transit·combo blocking·cap

`IncomingGarbagePacket`은 line 수, 명시적 hole column, `ready_at_frame`, hardened flag를 가진다. `after_travel`은 송신 frame에 20을 checked-add해 준비 frame을 만든다.

현재 TL의 combo blocking에서는 한 줄이라도 clear한 placement가 garbage rise를 전부 막는다. line clear가 없을 때만 준비된 packet을 queue 순서로 꺼내며, 한 placement에서 최대 8줄을 삽입한다. 20-frame gate 이전 packet은 queue에는 존재해 상쇄될 수 있지만 삽입되지 않는다.

hole 생성 자체는 아직 외부의 결정론적 scheduler가 공급한다. current client의 change-on-attack `messiness_change=1`, `messiness_inner=0`, 동일 column 재추첨 가능성과 packet이 완전히 상쇄됐을 때의 RNG 소비까지 고정한 generator는 후속 범위다. 이를 임의 RNG로 채우지 않는다.

## 5. Board provenance

`Board`는 다음 두 10×40 bit layer를 같은 index로 보관한다.

- `rows`: 모든 occupied cell
- `garbage_rows`: occupied cell 중 garbage인 cell

`garbage_rows[y]`는 항상 `rows[y]`의 부분집합이다. garbage 한 줄을 밀어 넣으면 두 layer가 함께 위로 이동하고 새 줄의 hole을 제외한 9칸이 두 layer 모두에 설정된다. line clear compaction도 두 layer를 함께 이동한다.

lock 결과의 `cleared_garbage`는 지워진 full row 중 garbage bit가 하나라도 있었는지 직접 계산한다. 이 값은 `PlacementOutcome`과 `AttackContext`로 전달되어 difficult garbage clear의 post-rounding `+1` 근거가 된다. replay checksum도 provenance 차이를 상태 차이로 취급한다.

## 6. API와 검증

- `GarbageRules`: transit 20, cap 8, opener 14, combo blocking
- `IncomingGarbageQueue`: ordered enqueue, pending/ready count, hardened-aware cancellation
- `cancel_attack_packets`: packet별 opener 재평가와 잔여 packet 보존
- `insert_ready_garbage`: transit, clear blocking과 cap 적용
- `Board::push_garbage_line`: 규칙 중립적인 layer shift와 40-row buffer overflow 보고

검증은 opener 14/15 boundary, ordered multi-packet 상쇄, transit 전 zero-passthrough 상쇄, hardened skip, 20-frame 준비 경계, line-clear blocking, 8-line cap, packet hole 순서, provenance compaction을 포함한다.

## 7. 남은 1 대 1 mechanics

1. change-on-attack hole RNG와 packet 소진/상쇄 시 RNG 소비
2. 180초 garbage margin 이후 multiplier 증가 `0.008/s`
3. 두 player frame scheduler에서 attack 전달·lock·tank 순서 통합
4. garbage rise 이후 Clutch Clear, block-out/garbage-out와 simultaneous terminal
5. reference fixture differential을 통한 `OBSERVED`에서 `CONFIRMED` 승격

현재 구현은 queue·상쇄·기본 삽입 전이를 실행할 수 있지만, 위 항목이 끝나기 전에는 전체 TL 1 대 1 mechanics 완료본이 아니다.
