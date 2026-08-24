# Local TETR.IO Mechanics Engine

TETR.IO의 학습 관련 mechanics를 독립적으로 재현하고 1 대 1 bot을 연구하기 위한 로컬 프로젝트다. 현재는 **Phase 1 deterministic core 및 Phase 2 timing 기반 구현 단계**이며 live TETR.IO 서비스에는 연결하지 않는다.

## 현재 구현 범위

- 10×40 row bitboard와 line clear
- 7종 tetromino의 4개 orientation
- seedable MINSTD 기반 generic 7-bag
- configurable spawn, next queue와 hold
- 공개 문서 기반 SRS+/180 kick 후보 table
- 실제 translation/rotation 성공 여부를 이용한 geometric reachable-lock 열거
- 유리수 누산 방식의 float 없는 frame gravity와 ordered input 처리
- configurable lock delay, lateral/rotation reset 및 reset cap
- field별 출처·확신도를 가진 버전 고정 `rules-tetrio` draft와 미확정 profile 활성화 차단

현재 구현은 target profile의 기반이지 아직 `TETR.IO BETA 1.7.8 / TL S2` conformance 완료본이 아니다. spawn/RNG/kick literal과 timing은 current fixture로 검증되기 전까지 `OBSERVED` 또는 `UNCONFIRMED`다. 현재 pinned draft는 필수 timing literal 6개가 비어 있으므로 의도적으로 실행 profile로 변환되지 않는다.

## 컨테이너 검증

```text
docker compose build rust
docker compose run --rm rust cargo fmt --all --check
docker compose run --rm rust cargo clippy --workspace --all-targets -- -D warnings
docker compose run --rm rust cargo test --workspace --all-targets
docker compose run --rm rust cargo build --workspace --release
```

설계와 조사 자료는 `PROJECT_PLAN_KO.md`, `.agent/`와 `research/`에서 확인할 수 있다.
