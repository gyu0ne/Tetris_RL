# Local TETR.IO Mechanics Engine

TETR.IO의 학습 관련 mechanics를 독립적으로 재현하고 1 대 1 bot을 연구하기 위한 로컬 프로젝트다. 솔로 플레이는 무점수 테스트 sandbox이며 40 LINES·BLITZ 점수 체계는 구현하지 않는다. live TETR.IO 서비스에는 연결하지 않는다.

## 현재 구현 범위

- 10×40 row bitboard와 line clear
- 7종 tetromino의 4개 orientation
- current-client MINSTD seed semantics와 `ZLOSIJT` 원본 순서를 재현하는 독립 piece/garbage RNG stream
- configurable spawn, next queue와 hold
- current-client SRS+/180 kick table, direct/fallback kick index와 T Mini→Full index 3 승격
- 실제 translation/rotation 성공 여부를 이용한 geometric reachable-lock 열거
- 백만분의 1칸 fixed-point 낙하 위상, spawn `0.96`, kick `0.1`, frame별 선형 gravity 증가와 20G cap
- configurable lock delay, lateral/rotation reset 및 reset cap
- ordered input edge, DAS/ARR/DCD 및 sonic drop을 discrete action으로 바꾸는 frame-aligned sandbox handling normalizer
- held-key IRS/IHS buffer와 generic IHS→IRS spawn 적용
- 입력 정규화부터 hold, 이동/회전, 중력, lock/line clear, 다음 spawn까지 이어지는 결정론적 `FrameSession`
- 마지막 성공 입력, 회전 방향과 kick index를 보존하는 spin provenance
- All-Mini+의 T corner/immobility 및 non-T immobility 판정, post-clear perfect clear
- `BlockOut`/`LockOut`/`PartialLockOut`을 구분하는 typed top-out 규칙과 lock visibility
- field별 출처·확신도를 가진 버전 고정 `rules-tetrio` profile과 실행 가능/기능 동등성 검증 상태 분리
- canonical solo/1대1 snapshot, 최초 불일치 component와 필수 mechanics coverage를 판정하는 `replay-conformance` crate
- 점수와 분리된 `ClearEvent` 및 TL base attack, multiplier combo, B2B/Surge, Perfect Clear와 garbage-clear +1을 계산하는 `versus` crate; client의 `Math.log1p`와 반복 `+=`가 관찰 가능한 경로만 guarded IEEE-754로 재현
- current client 순서를 보존하는 fixed-capacity attack packet: Surge 최대 3개 → clear → Perfect Clear
- change-on-attack garbage hole RNG, transit/cancel/cap/combo-blocking/instant insertion과 margin multiplier
- lock→상쇄→동시 zero-passthrough→garbage→spawn 순서를 보존하는 결정론적 2인 `BattleSession`
- BlockOut/GarbageOut, Clutch Clear, 단독 승패와 동시 사망 draw

선언된 학습 mechanics의 실행 경로는 `TETR.IO BETA 1.7.8 / TL S2` current client asset을 기준으로 구현되어 있다. 여기에는 `0.02G`, 120초 이후 gravity 증가, client의 `locking > locktime`/reset-cap 경계, 공격·garbage·Clutch·round terminal 순서가 포함된다. 다만 기준 board/attack/garbage checkpoint corpus가 없으므로 profile 표기는 계속 `OBSERVED_NOT_FUNCTIONALLY_VERIFIED`다. 운영자의 승인이나 공식 인증은 요구하지 않는다. 같은 조건·입력의 version-pinned reference trace와 exact diff가 0이고 필수 mechanics claim 및 기본 10,000개 randomized battle case가 모두 통과하면 `Conformant`로 판정한다. TL은 room handling을 강제하지 않으므로 DAS/ARR/DCD/SDF는 player/replay config로 공급한다. 브라우저 OS event의 0.1 subframe 재생은 검증 adapter 범위이며, 주 학습 action인 reachable locked afterstate와 1 대 1 상태 전이는 raw keyboard timestamp에 의존하지 않는다. 사용자 제공 BLITZ replay는 이 입력 형식 검증에만 쓰고 replay player/viewer는 만들지 않는다.

## 컨테이너 검증

```text
docker compose build rust
docker compose run --rm rust cargo fmt --all --check
docker compose run --rm rust cargo clippy --workspace --all-targets -- -D warnings
docker compose run --rm rust cargo test --workspace --all-targets
docker compose run --rm rust cargo build --workspace --release
```

설계와 조사 자료는 `PROJECT_PLAN_KO.md`, `.agent/`와 `research/`에서 확인할 수 있다.
