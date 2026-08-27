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
- SHA-256으로 원본/trace provenance를 분리한 normalized JSON v1 loader, canonical solo/1대1 snapshot, 최초 불일치 component와 필수 mechanics coverage를 판정하는 `replay-conformance` crate
- 브라우저 입력을 별도 JS 게임이 아니라 실제 `FrameSession`에 전달하는 `manual-playground`
- 점수와 분리된 `ClearEvent` 및 TL base attack, multiplier combo, B2B/Surge, Perfect Clear와 garbage-clear +1을 계산하는 `versus` crate; client의 `Math.log1p`와 반복 `+=`가 관찰 가능한 경로만 guarded IEEE-754로 재현
- current client 순서를 보존하는 fixed-capacity attack packet: Surge 최대 3개 → clear → Perfect Clear
- change-on-attack garbage hole RNG, transit/cancel/cap/combo-blocking/instant insertion과 margin multiplier
- lock→상쇄→동시 zero-passthrough→garbage→spawn 순서를 보존하는 결정론적 2인 `BattleSession`
- BlockOut/GarbageOut, Clutch Clear, 단독 승패와 동시 사망 draw
- hold 분기를 포함한 모든 geometric locked afterstate를 평가하고 Dellacherie 계열 정수 feature/점수/순위를 생성하는 `arena` crate
- 후보마다 공유되는 CPU용 `10→64→32→1` afterstate scorer, match/seed split 검증 loader와 listwise teacher-score distillation
- 매 epoch 순서 샘플링, best-epoch 복원, early stopping과 서로 다른 초기화 seed 3개를 사용하는 장기 모방학습
- 실제 `model.pt`를 Rust 엔진에 되먹임하는 후보 선택·closed-loop 생존 평가·checkpoint 승격
- 교사 라벨 부가 계산을 제거한 추론 전용 Rust 후보 경로와 seed-sharded closed-loop 병렬 평가
- 모델이 방문한 상태에 휴리스틱 교사 점수를 다시 붙이는 learner-state dataset aggregation
- 승격된 실제 checkpoint의 착지 선택을 재생·일시정지·단일 진행할 수 있는 로컬 모델 관전자
- 진행 중 경기와 상대 배정을 update 경계에서 정확히 복원하고 normalized-entropy PPO, 상대 풀, 기술·공격 진단을 제공하는 1대1 자기대전 학습기

선언된 학습 mechanics의 실행 경로는 `TETR.IO BETA 1.7.8 / TL S2` current client asset을 기준으로 구현되어 있다. 여기에는 `0.02G`, 120초 이후 gravity 증가, client의 `locking > locktime`/reset-cap 경계, 공격·garbage·Clutch·round terminal 순서가 포함된다. version-pinned capture를 strict JSON과 정확한 SHA-256으로 읽는 adapter 경계도 구현했다. 다만 실제 기준 board/attack/garbage checkpoint corpus가 없으므로 profile 표기는 계속 `OBSERVED_NOT_FUNCTIONALLY_VERIFIED`다. 이 표시는 픽셀이나 UI가 다르다는 뜻이 아니라 아직 외부 mechanics corpus를 모두 채우지 않았다는 뜻이다. 운영자의 승인이나 공식 인증은 요구하지 않는다. formal report에서는 같은 조건·입력의 version-pinned reference trace와 exact diff가 0이고 필수 mechanics claim 및 기본 randomized battle 하한이 모두 통과하면 `Conformant`로 판정한다. 이 formal label은 수동 테스트와 heuristic/모방학습 도구 개발을 막지 않는다. TL은 room handling을 강제하지 않으므로 DAS/ARR/DCD/SDF는 player/replay config로 공급한다. 브라우저 OS event의 0.1 subframe 재생은 검증 adapter 범위이며, 주 학습 action인 reachable locked afterstate와 1 대 1 상태 전이는 raw keyboard timestamp에 의존하지 않는다.

## 직접 테스트

다음 명령으로 로컬 solo playground를 실행한다.

```text
docker compose up --build playground
```

브라우저에서 `http://127.0.0.1:8787`을 열면 된다. 방향키 이동/soft drop, `Space` hard drop, `Z`/`X` 회전, `A` 180도, `C` hold, `P` 일시정지, `N` single-frame을 지원한다. 화면은 locked block type까지 복제하지 않는 진단용 표시이며, 입력·중력·회전·kick·hold·lock·line clear·spin·perfect clear·top-out은 실제 Rust `FrameSession`이 계산한다.

종료할 때는 다음을 실행한다.

```text
docker compose stop playground
```

## 첫 모방학습 smoke

학습 action은 key sequence가 아니라 `hold + piece + orientation + x/y` 착지점이다. 이동 경로 길이는 진단 정보일 뿐 모델 입력이 아니다. 다음 명령은 512개 결정의 임시 teacher shard를 만들고 2,817-parameter scorer를 학습한다. `OBSERVED` mechanics로 하는 탐색 실행이므로 `--allow-observed`를 명시해야 한다.

```text
docker compose run --rm rust cargo run --release -p arena --bin generate-solo -- --records datasets/solo-imitation-smoke-v1/records.jsonl.gz --manifest datasets/solo-imitation-smoke-v1/manifest.json --engine-revision <GIT_REVISION> --seed 1 --matches 8 --decisions-per-match 64
docker compose build training
docker compose run --rm training python -m tetris_rl.training.imitation --manifest datasets/solo-imitation-smoke-v1/manifest.json --output checkpoints/solo-imitation-smoke-v1/model.pt --epochs 3 --batch-decisions 32 --allow-observed
```

dataset shard는 deterministic gzip 임시 파일이며 저장소에 commit하지 않는다. 체크포인트가 manifest, feature 정규화, rules/engine/teacher hash와 학습 설정을 자체 포함하므로 학습과 offline 평가 후 shard는 삭제하고 같은 config/seed로 재생성할 수 있다. 현재 solo teacher는 board feature bootstrap용이고 1대1 base policy 성능을 뜻하지 않는다.

최종 solo bootstrap은 4,096개의 서로 다른 game seed, 최소 100만 teacher decision, 독립 초기화 3회, 최대 100 epoch와 2,000만 placement 생존 평가를 사용한다. 아래 명령 하나가 생성→학습→후보 선택→장기 평가→필요 시 learner-state 재학습→승격을 자동으로 수행한다.

```powershell
./scripts/run-final-solo-bootstrap.ps1
```

실행 절차와 중간 산출물은 `Explanation/Imitation_Learning_Runbook.md`, 평가기와 승격 구조는 `Explanation/Imitation_Model_Evaluation_and_Promotion.md`에 있다. 모델 가중치용 휴대용 JSON은 사용하지 않는다.

## 1대1 자기대전 학습

r0 진단을 반영한 v2 학습은 현재 작업공간에 r0 update 50 모델이 있으면 이를 초기화에 사용하고 새 r1 실험을 만든다. 해당 파일이 없는 새 checkout에서는 승격된 솔로 bootstrap으로 시작한다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile max -Hours 24
```

중단 후에는 `./scripts/run-versus-selfplay.ps1 -ResourceProfile max -Hours 24 -Resume`으로 `latest.pt`의 모델·optimizer·진행 경기·고정 상대 배정을 이어간다. 보조 보상, entropy 수정, update 로그와 paired 평가 방법은 `Explanation/Versus_Self_Play_Reinforcement_Learning.md`에 정리되어 있다.

## 최종 모델 관전

승격된 최종 checkpoint를 실제 Rust 착지 엔진에서 실행해 보려면 다음을 사용한다.

```text
docker compose up --build spectator
```

브라우저에서 `http://127.0.0.1:8788`을 연다. 재생·일시정지·1수 진행, 속도 변경, seed 초기화를 지원한다. 보드는 학습 action과 같은 도달 지점 단위이며 화면의 후보 수·선택 점수·추론 시간은 현재 `model.pt`를 직접 채점한 결과다. 종료 명령은 `docker compose stop spectator`다. 상세 구조는 `Explanation/Solo_Model_Spectator_and_Fast_Evaluation.md`에 있다.

## 컨테이너 검증

```text
docker compose build rust
docker compose run --rm rust cargo fmt --all --check
docker compose run --rm rust cargo clippy --workspace --all-targets -- -D warnings
docker compose run --rm rust cargo test --workspace --all-targets
docker compose run --rm rust cargo build --workspace --release
docker compose build training
docker compose run --rm training ruff format --check --config python/pyproject.toml python
docker compose run --rm training ruff check --config python/pyproject.toml python
docker compose run --rm training python -m unittest discover -s python/tests -v
```

구현된 기능별 한국어 설명서는 `Explanation/`에서 확인할 수 있다. 설계 결정과 조사 자료는 `PROJECT_PLAN_KO.md`, `.agent/`와 `research/`에 보관한다.
