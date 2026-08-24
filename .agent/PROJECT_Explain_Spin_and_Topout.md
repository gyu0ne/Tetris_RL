# Spin, Perfect Clear 및 Top-out 설계 설명

상태: generic mechanics foundation 구현 완료, target edge conformance 미완료
기준 시각: `2026-08-24T17:34:50+09:00`

## 1. 이번 단계에 새 웹 조사가 필요하지 않은 이유

마지막 성공 입력 보존, line clear 뒤 빈 보드 검사와 top-out 사유의 typed representation은 외부 상수 문제가 아니라 엔진 상태 불변식이다. All-Mini+의 기본 분기 역시 이미 source ledger에 기록된 BETA 1.5.0 변경과 조사 문서의 T/non-T immobility 경로를 사용했다. 따라서 이번 단계는 새 웹 검색 없이 기존 근거를 코드로 옮겼다.

다만 exact T kick-index upgrade, lock-out 우선순위와 Clutch Clear displacement는 기존 근거만으로 확정할 수 없다. 이 값은 구현을 막는 이유가 아니라 profile별 미확정 경계이며, fixture 없이 `CONFIRMED`로 승격하지 않는다.

## 2. 마지막 성공 행동과 회전 provenance

`TimingState::last_action`은 `None`, lateral translation, soft drop, 실제 이동한 hard drop, rotation으로 구분한다. rotation은 방향과 ordered kick table index를 함께 저장한다.

- 실패한 입력은 마지막 성공 행동을 바꾸지 않는다.
- 자동 gravity는 player action이 아니므로 회전 provenance를 지우지 않는다.
- hard drop이 실제로 행을 이동하면 `HardDrop`으로 바뀐다.
- 이미 grounded인 zero-distance hard drop은 직전 rotation을 보존한다.
- 새 piece와 hold 교체는 기본적으로 `None`에서 시작하고, 성공한 IRS는 rotation provenance로 시작한다.

이 상태는 `FrameOutcome`, `FrameSession`, `TimingSnapshot`까지 보존되므로 spin 판정과 differential trace가 같은 정보를 사용한다.

## 3. All-Mini+ generic classifier

spin은 board lock 전에 다음 입력을 받는다.

```text
spin = classify(board_before_lock, final_piece, last_action, spin_rules)
```

마지막 성공 행동이 rotation이 아니면 spin이 아니다. All-Mini+ mode의 구현 경로는 다음과 같다.

1. non-T piece가 상하좌우로 모두 이동 불가능하면 Mini다.
2. T piece의 회전 중심 주위 네 corner 중 3개 이상이 막혀 있고 방향 기준 두 front corner가 모두 막혀 있으면 Full이다.
3. 3-corner지만 front corner 조건이 부족하면 Mini다.
4. 3-corner를 만족하지 않아도 T가 immobile이면 BETA 1.5.0 경로에 따라 Mini다.

T Mini를 Full로 올리는 kick index는 `t_full_kick_upgrade_mask`로 외부화했다. 현재 target의 exact mask는 fixture가 없어 0인 provisional 값이며, test는 명시적 mask가 주어졌을 때 upgrade가 동작함을 확인한다. 공격량과 B2B 계산은 이후 versus layer 책임이다.

## 4. Perfect clear

`Board::lock`은 piece 기록 후 full row를 압축하고, 그 결과 모든 40개 row가 0인지 검사한다. 따라서 perfect clear는 lock 전이나 clear 전 occupied-cell 수로 추측하지 않고 **line clear가 끝난 최종 board**에서만 true가 된다.

## 5. Typed top-out

`TopOutReason`은 다음을 구분한다.

- `BlockOut`: 새 active piece가 board와 충돌해 timing kernel에 들어갈 수 없음
- `LockOut`: lock한 네 cell이 모두 hidden 영역에 있음
- `PartialLockOut`: visible/hidden 영역에 걸쳐 lock됨

`Board::lock`은 clear 전 piece cell을 기준으로 `Visible`, `PartiallyHidden`, `FullyHidden`을 기록한다. `TopOutRules`가 이 정보를 terminal reason으로 승격한다. 현재 기본값은 기존에 구현돼 있던 block-out만 활성화하며, target에서 확인되지 않은 lock-out과 partial lock-out을 임의로 켜지 않는다.

Clutch Clear, line-clear와 spawn 충돌의 우선순위 및 next-piece upward displacement는 아직 구현하지 않았다. 이 부분을 “정확히 구현 완료”라고 주장하지 않는다.

## 6. 검증

추가 test는 다음 경계를 포함한다.

- rotation direction/kick 보존 및 translation 덮어쓰기
- 이동 hard drop과 zero-distance hard drop의 provenance 차이
- T Full, T Mini, immobile non-T Mini, non-rotation 거부
- 명시적 kick mask에 의한 T Full upgrade
- line clear 뒤 perfect clear
- visible/partial/full-hidden lock 구분
- block-out 사유와 선택 가능한 lock-out 변형
- `rules-tetrio` All-Mini+ profile의 core rule mapping

다음 단계는 line-clear event/scoring이며, exact target spin/top-out edge는 별도의 differential fixture gate로 남는다.
