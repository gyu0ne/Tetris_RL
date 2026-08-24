# Local TETR.IO Mechanics Engine

TETR.IO의 학습 관련 mechanics를 독립적으로 재현하고 1 대 1 bot을 연구하기 위한 로컬 프로젝트다. 솔로 플레이는 무점수 테스트 sandbox이며 40 LINES·BLITZ 점수 체계는 구현하지 않는다. live TETR.IO 서비스에는 연결하지 않는다.

## 현재 구현 범위

- 10×40 row bitboard와 line clear
- 7종 tetromino의 4개 orientation
- seedable MINSTD 기반 generic 7-bag
- configurable spawn, next queue와 hold
- 공개 문서 기반 SRS+/180 kick 후보 table
- 실제 translation/rotation 성공 여부를 이용한 geometric reachable-lock 열거
- 유리수 누산 방식의 float 없는 frame gravity와 ordered input 처리
- configurable lock delay, lateral/rotation reset 및 reset cap
- ordered input edge, DAS/ARR/DCD 및 sonic drop을 discrete action으로 바꾸는 generic handling normalizer
- held-key IRS/IHS buffer와 generic IHS→IRS spawn 적용
- 입력 정규화부터 hold, 이동/회전, 중력, lock/line clear, 다음 spawn까지 이어지는 결정론적 `FrameSession`
- 마지막 성공 입력, 회전 방향과 kick index를 보존하는 spin provenance
- All-Mini+의 T corner/immobility 및 non-T immobility 판정, post-clear perfect clear
- `BlockOut`/`LockOut`/`PartialLockOut`을 구분하는 typed top-out 규칙과 lock visibility
- field별 출처·확신도를 가진 버전 고정 `rules-tetrio` profile과 실행 가능/동등성 인증 상태 분리
- canonical frame snapshot과 최초 불일치 component를 반환하는 `replay-conformance` crate
- 점수와 분리된 `ClearEvent` 및 정수-only TL base attack, multiplier combo, B2B/Surge, Perfect Clear와 garbage-clear +1을 계산하는 `versus` crate
- current client 순서를 보존하는 fixed-capacity attack packet: Surge 최대 3개 → clear → Perfect Clear

현재 구현은 target profile의 기반이지 아직 `TETR.IO BETA 1.7.8 / TL S2` conformance 완료본이 아니다. 두 client asset snapshot에서 31개 TL option이 동일함을 확인해 `0.02G`, 30-frame lock, 15 resets, ARE 0 등의 timing 값을 `OBSERVED`로 채웠다. current asset에서 53개 firepower case를 다시 생성해 공격표와 packet 순서를 구현했고, direct client control-flow 관찰을 근거로 ordered incoming queue, packet별 상쇄, 14-piece opener double-cancel, 20-frame transit gate, combo blocking, 8-line cap 및 garbage provenance 삽입까지 `OBSERVED`로 구현했다. All-Mini+ 기본 판정 경로는 구현했지만 exact T kick-index upgrade, change-on-attack hole RNG, margin multiplier, Clutch Clear와 target top-out/round-terminal 우선순위는 fixture 전까지 미완료 또는 `UNCONFIRMED`다. TL은 room handling을 강제하지 않으므로 DAS/ARR/DCD/SDF는 player/replay config로 별도 공급한다. 사용자 제공 BLITZ replay는 입력 형식과 handling 검증 자료로만 사용하며, 제품 기능인 replay player/viewer는 구현하지 않는다.

## 컨테이너 검증

```text
docker compose build rust
docker compose run --rm rust cargo fmt --all --check
docker compose run --rm rust cargo clippy --workspace --all-targets -- -D warnings
docker compose run --rm rust cargo test --workspace --all-targets
docker compose run --rm rust cargo build --workspace --release
```

설계와 조사 자료는 `PROJECT_PLAN_KO.md`, `.agent/`와 `research/`에서 확인할 수 있다.
