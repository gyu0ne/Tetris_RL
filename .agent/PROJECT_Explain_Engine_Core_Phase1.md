# Phase 1 결정론적 엔진 Core 구현

상태: 첫 구현 완료, target conformance 미완료

구현일: `2026-08-24`

## 구현한 경계

`crates/engine-core`에 rendering, network, wall clock과 학습 코드를 포함하지 않는 mechanics kernel을 만들었다.

- `board.rs`: 10×40 row bitboard, collision, lock, 최대 4줄 clear와 stable checksum
- `piece.rs`: 7종 tetromino, 4 orientation, SRS-compatible 4×4 좌표
- `rng.rs`: seed normalization이 명시된 MINSTD와 pre-shuffle order를 주입할 수 있는 7-bag
- `rotation.rs`: ordered SRS/SRS+ quarter/half kick 적용
- `reachability.rs`: 실제 translation/rotation 성공을 따라가는 BFS geometric reachable-lock 열거와 shortest path
- `game.rs`: configurable spawn, preview queue, piece당 1회 hold, reachable placement lock과 top-out 상태

## 중요한 제한

이 구현은 target profile의 기반이지 TETR.IO conformance 완료본이 아니다.

- `SpawnRules::modern_observed()`의 origin은 fixture 전까지 `OBSERVED`다.
- `SevenBag::new()`의 canonical order와 MINSTD 사용은 generic default다. current TETR.IO RNG라는 주장이 아니다.
- SRS+ I kick ordering과 180 table은 공개 구현·문서 기반 `OBSERVED` 값이며 current fixture가 필요하다.
- `reachable_locks`는 gravity, DAS/ARR/DCD, lock delay와 frame budget을 반영하지 않는 geometric reachability다.
- spin/All-Mini+, scoring, attack, garbage와 round terminal은 아직 구현하지 않았다.

이 제한을 API 이름과 문서에 직접 남겨 provisional 동작을 target 동등성으로 오인하지 않게 했다.

## 좌표·결정론 규약

- board의 `y=0`이 바닥이며 위쪽이 양수다.
- board row는 하위 10 bit만 사용한다.
- piece의 `x,y`는 SRS 4×4 local box의 왼쪽 아래다.
- authoritative state transition에는 float를 사용하지 않는다.
- BFS action order와 `BTreeSet/BTreeMap` ordering을 고정해 같은 입력에서 같은 placement/path 순서를 만든다.
- replay checkpoint용 board checksum은 stable FNV-1a이며 보안 hash로 사용하지 않는다.

## 현재 검증

- container: `rust:1.89.0-slim-bookworm`
- `cargo test --workspace --all-targets`: 22 passed, 0 failed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed
- `cargo fmt --all --check`: passed
- `cargo build --workspace --release`: passed
- test 범위: bit mask 검증, row compaction, collision, piece geometry, RNG/7-bag permutation과 replay, SRS+ kick order, wall kick, empty-board T placement 34개, path replay, hold restriction, 동일 seed/choice state reproduction

이 결과는 `rust:1.89.0-slim-bookworm` image에서 재현했으며 continuity outcome에도 기록한다.

## 다음 엔진 단계

1. target fixture 형식과 versioned `rules-tetrio` profile 생성
2. spawn/RNG/kick literal differential test
3. frame input ordering, handling, gravity, lock/reset과 timing-aware reachability
4. spin/All-Mini+, clear event와 attack 계산
5. 2인 garbage/round terminal

모델용 기록 생성은 위 mechanics 중 label에 영향을 주는 부분의 conformance가 끝난 뒤 시작한다.
