# PROJECT Explain: 솔로 학습 완성 계획

작성 시각: `2026-08-25T20:26:31+09:00`

상태: **계획 확정 / 장기 학습 실행 전**

## 1. 이번 범위

현재 프로젝트의 완료 목표는 **로컬 엔진에서 혼자 장시간 생존하는 솔로 모델을 실제로 생성하고 검증하는 것**까지다. 1대1 강화학습 환경, self-play, 상대 모델, 랭킹 평가는 이번 범위에 포함하지 않는다. 이미 구현된 1대1 mechanics는 엔진 자산으로 보존하지만, 1대1 학습 코드는 사용자가 다시 지시하기 전까지 추가하지 않는다.

40 LINES, BLITZ 점수, 라이브 TETR.IO 접속, 픽셀 단위 UI 재현과 운영자 인증도 완료 조건이 아니다.

## 2. 현재 기준선

다음 구성은 commit `82f845f`에 구현되어 있다.

- 휴리스틱 교사가 모든 legal afterstate를 평가해 압축 records와 manifest를 생성한다.
- Python/PyTorch의 `10→64→32→1` scorer가 교사의 전체 후보 순위를 모방한다.
- 서로 다른 초기화 seed 세 개를 독립 학습하고 실제 `.pt` 후보를 비교한다.
- offline 평가와 Rust 엔진 기반 closed-loop 솔로 평가를 모두 통과한 후보만 승격한다.
- 실패하면 모델이 실제 방문한 상태를 휴리스틱으로 다시 라벨링하는 learner-state aggregation을 최대 두 번 수행한다.
- 승격된 단일 checkpoint에는 모델 구조, 정규화, 데이터·엔진 provenance와 평가 결과가 포함된다.

파이프라인 축소판은 끝까지 실행해 검증했지만, 하루 규모의 실제 장기 학습과 최종 모델 생성은 아직 수행하지 않았다.

## 3. 완료 기준

솔로 학습은 다음 조건을 모두 충족할 때 완료한다.

1. clean commit에서 컨테이너 이미지가 재현 가능하게 build된다.
2. 최소 1,000,000개의 교사 decision을 4,096개 이상의 서로 다른 게임 seed에서 생성한다.
3. 초기화 seed `2026`, `2027`, `2028`로 세 후보를 학습한다.
4. 각 후보는 최소 20 epoch를 학습하고, 최대 100 epoch 안에서 validation teacher regret 기반 early stopping과 best-epoch 복원을 사용한다.
5. 학습에 사용하지 않은 256개 development seed에서 2,000 placement 동안 top-out이 없는 후보만 최종 평가 대상으로 삼는다.
6. 별도의 2,000개 final seed에서 게임당 10,000 placement, 총 20,000,000 placement를 실행하고 top-out이 한 번도 없어야 한다.
7. 최종 파일 `checkpoints/solo-imitation-versus-bootstrap-v1/model.pt`를 새 프로세스에서 단독으로 다시 불러오고 metadata와 promotion 증거를 검증한다.

여기서 “혼자서는 죽지 않는다”는 모든 가능한 상태에 대한 수학적 불사 증명이 아니라, 명시한 미사용 seed 2,000개와 2,000만 placement에서 top-out 0회를 뜻한다. 이 기준을 낮춰 모델을 강제로 통과시키지 않는다.

## 4. 실행 단계

### 단계 A — 소스와 환경 고정

- 작업 트리가 clean인지 확인한다.
- `git rev-parse HEAD`를 dataset의 `engine_revision`으로 사용한다.
- Rust와 training 이미지를 다시 build한다.
- source, config 또는 dependency가 바뀌면 기존 dataset/checkpoint와 섞지 않고 새 revision으로 처음부터 실행한다.

완료 조건: build 성공과 revision 기록.

### 단계 B — 휴리스틱 시연 생성

- `base_seed=10001`, `seed_stride=104729`의 결정론적 schedule을 사용한다.
- 4,096게임 × 최대 250 decision으로 시작한다.
- 실제 decision 수가 1,000,000보다 작으면 4,224게임 설정으로 전체 shard를 다시 생성한다.
- records SHA-256, rules ID, teacher ID, seed schedule과 decision 수를 manifest에 기록한다.

완료 조건: 무결성 검사를 통과한 decision 1,000,000개 이상.

### 단계 C — 세 후보 모방학습

- 세 초기화 seed를 같은 train/validation 분할에서 독립 학습한다.
- 매 epoch bounded-memory shuffle을 사용하고 마지막 epoch가 아니라 validation regret 최저 checkpoint를 보존한다.
- 매 epoch 종료 시 현재 모델·optimizer·best 모델·early-stopping 상태를 원자적으로 저장하며, 중단 후 정확히 다음 epoch부터 재개한다. 완료된 독립 seed 후보는 호환성 검증 후 건너뛴다.
- `light`, `balanced`, `max`는 독립 seed 병렬도와 native thread 수만 조절한다. 학습 의미를 바꾸는 batch·learning rate·seed·epoch 설정은 프로필과 무관하게 고정한다.
- 모델 크기는 현재 2,817 parameters를 유지한다. 장기 실행 결과가 구조적 한계를 보여주기 전에는 CNN이나 대형 모델로 확장하지 않는다.

완료 조건: 세 개의 load 가능한 `.pt` 후보와 학습 metadata 생성.

### 단계 D — 후보 선택과 장기 생존 평가

- tie-aware offline 지표로 교사 순위 모방 품질을 확인한다.
- 256개 development seed의 closed loop에서 생존율 1.0인 후보 중 최선의 모델을 선택한다.
- 선택에 사용하지 않은 2,000개 final seed에서 2,000만 placement를 실행한다.

완료 조건: illegal action 0회, final top-out 0회, 평가 보고서의 checkpoint/dataset/revision hash 일치.

### 단계 E — 실패 시 learner-state 보강

- final top-out이 한 번이라도 발생하면 실패 상태를 숨기거나 gate를 낮추지 않는다.
- 선택된 모델이 방문한 상태 250,000개를 다시 휴리스틱으로 라벨링해 기존 dataset과 결합한다.
- 서로 다른 evaluation seed 집합으로 세 후보를 처음부터 재학습한다.
- 최대 두 번 반복한다.

완료 조건: 장기 생존 gate 통과. 두 번의 보강 후에도 실패하면 실행을 중단하고 feature/model 용량 또는 교사 자체를 재설계한다.

### 단계 F — 승격과 인계

- 최종 checkpoint와 offline/selection/closed-loop 보고서의 SHA-256 및 provenance를 교차 검증한다.
- 검증 결과를 self-contained checkpoint에 내장한다.
- 새 컨테이너 프로세스에서 checkpoint 단독 load를 확인한다.
- 생성 records와 중간 후보는 최종 load 확인 후에만 선택적으로 정리한다.

완료 조건: `checkpoints/solo-imitation-versus-bootstrap-v1/model.pt` 단독 재사용 성공.

## 5. 실행 명령

전체 단계는 clean commit에서 다음 한 명령으로 실행한다.

```powershell
./scripts/run-final-solo-bootstrap.ps1
```

CPU thread 수만 조정하려면 다음처럼 실행한다.

```powershell
./scripts/run-final-solo-bootstrap.ps1 -ResourceProfile max -ReuseDataset
```

실행은 수 시간에서 하루 규모를 허용한다. 임의의 짧은 epoch나 적은 게임으로 완료 판정을 대신하지 않는다. 정확한 wall time은 현재 장비와 aggregation 발생 여부에 따라 달라지므로 실행 전에는 확정값으로 약속하지 않는다.

## 6. 산출물과 보존 정책

최종 필수 산출물은 다음 하나다.

```text
checkpoints/solo-imitation-versus-bootstrap-v1/model.pt
```

재현·진단 중에는 다음 중간 산출물을 유지한다.

- `datasets/solo-imitation-bootstrap-v1/`
- 필요 시 `datasets/solo-imitation-dagger-r1/`, `r2/`
- `checkpoints/solo-imitation-bootstrap-r0/`, `r1/`, `r2/`
- `runs/evaluation/solo-imitation-r*-*.json`

이 파일들은 Git에 commit하지 않는다. 최종 checkpoint의 단독 load와 내장된 hash 검증이 끝난 뒤에만 삭제를 고려한다.

## 7. 중단 및 재설계 조건

다음 중 하나가 발생하면 약한 모델을 승격하지 않고 원인을 분류한다.

- 데이터 또는 checkpoint hash/provenance 불일치
- illegal action 발생
- 세 후보 모두 development 생존율 1.0 미달
- 두 번의 learner-state aggregation 후에도 final top-out 발생
- 메모리 부족이나 컨테이너 비정상 종료로 checkpoint 원자성이 보장되지 않음

처음 세 항목은 data/evaluator 결함 여부를 먼저 확인한다. 마지막 장기 생존 실패가 재현되면 feature 표현, 교사 탐색 깊이, 모델 용량 순서로 재설계를 검토한다.

## 8. 이번 계획에서 보류하는 항목

- fixed-cadence 1대1 placement 환경
- 상대 보드·incoming garbage를 포함한 observation
- self-play 강화학습과 opponent pool
- 1대1 Elo/승률 평가 및 최종 대전 봇
- 외부 TETR.IO reference corpus를 이용한 정식 `Conformant` 보고서

이 항목들은 솔로 모델 생성 후 사용자가 범위를 다시 열 때 별도 계획으로 시작한다. 현재 다음 작업은 새로운 1대1 코드를 작성하는 것이 아니라 위 한 명령으로 장기 솔로 학습을 실행하는 것이다.
