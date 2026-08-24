# Spin, Perfect Clear 및 Top-out 설계 설명

상태: observed spin/Clutch/top-out 실행 구현 완료, 외부 differential 인증 미완료
기준 시각: `2026-08-24T20:29:16+09:00`

## 1. 근거 갱신

마지막 성공 입력, line clear 뒤 perfect clear와 typed top-out은 엔진 불변식으로 유지한다. 추가로 현재 client bundle의 회전·spin·Clutch 제어 흐름을 직접 대조해 kick 번호와 우선순위를 `OBSERVED`로 채웠다. 외부 기준 state fixture가 없으므로 `CONFIRMED`로 승격하지는 않는다.

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

T Mini를 Full로 올리는 kick index는 `t_full_kick_upgrade_mask`로 외부화했다. current client는 `falling.kick === 3`일 때 Full로 올리므로 observed target mask는 `1 << 3`이다. direct 회전과 첫 fallback은 모두 kick `0`, 이후 fallback은 `1..3`으로 저장해 upstream numbering과 맞춘다. 공격량과 B2B 계산은 versus layer 책임이다.

## 4. Perfect clear

`Board::lock`은 piece 기록 후 full row를 압축하고, 그 결과 모든 40개 row가 0인지 검사한다. 따라서 perfect clear는 lock 전이나 clear 전 occupied-cell 수로 추측하지 않고 **line clear가 끝난 최종 board**에서만 true가 된다.

## 5. Typed top-out

`TopOutReason`은 block/lock/partial-lock 계열을 구분하고, versus scheduler는 lethal garbage와 동시 terminal을 별도로 판정한다.

- `BlockOut`: 새 active piece가 board와 충돌해 timing kernel에 들어갈 수 없음
- `LockOut`: lock한 네 cell이 모두 hidden 영역에 있음
- `PartialLockOut`: visible/hidden 영역에 걸쳐 lock됨

`Board::lock`은 clear 전 piece cell을 기준으로 `Visible`, `PartiallyHidden`, `FullyHidden`을 기록한다. `TopOutRules`가 profile에서 활성화한 사유만 terminal로 승격한다. observed TL 기본은 block-out과 lethal garbage를 사용한다.

line clear 직후 spawn이나 IHS replacement가 막히면 Clutch가 piece를 위로 이동시켜 구조할 수 있다. lock 결과, Clutch 가능 여부, spawn/IHS와 최종 block-out 순서를 `GameState`와 `FrameSession`에 통합했다. 양쪽 terminal은 `BattleSession`이 같은 tick 결과를 모두 계산한 뒤 승/패/무승부로 확정한다.

## 6. 검증

추가 test는 다음 경계를 포함한다.

- rotation direction/kick 보존 및 translation 덮어쓰기
- 이동 hard drop과 zero-distance hard drop의 provenance 차이
- T Full, T Mini, immobile non-T Mini, non-rotation 거부
- upstream 번호 3 kick mask에 의한 T Full upgrade
- line clear 뒤 perfect clear
- visible/partial/full-hidden lock 구분
- block-out, lethal garbage, Clutch spawn/IHS 구조와 동시 terminal
- `rules-tetrio` All-Mini+ profile의 core rule mapping

score-free `ClearEvent`, observed versus attack/garbage 및 round terminal까지 연결됐다. exact target 인증은 별도의 differential fixture gate로 남는다.
