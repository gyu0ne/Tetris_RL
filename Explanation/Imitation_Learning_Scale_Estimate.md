# 모방학습 규모 이론 예측

작성일: `2026-08-25`

대상: `afterstate-scorer-v1` (`10 → 64 → 32 → 1`, 2,817 parameters)

## 결론

첫 번째 실전 규모는 **교사 의사결정 100,000개, batch 32, 최대 5 epoch**로 잡는다. 약 20%를 match/seed 단위 validation으로 분리하면 실제 gradient update는 약 `80,000 / 32 × 5 = 12,500`회다. 100,000개 상태에는 현재 후보 수 평균 68.60을 적용할 때 약 686만 개의 candidate score가 포함된다.

이 규모의 목적은 강한 1대1 봇 완성이 아니라 다음 수준의 **기본 솔로 stack policy**를 얻는 것이다.

- hold를 포함한 합법 afterstate 중 교사와 동등하거나 거의 동등한 착지 선택
- 보지 못한 seed에서 즉시 붕괴하지 않는 기본 생존·평탄화 능력
- 이후 1대1 imitation과 self-play RL을 시작할 수 있는 초기 가중치

100,000개 이후에도 closed-loop 성능이 부족하면 동일한 교사 trajectory를 300,000개 이상 반복하기보다 학습자 자신이 방문한 상태를 교사가 다시 평가하는 dataset aggregation으로 전환한다. 권장 후속은 `50,000개 × 2회`이고, 따라서 첫 bootstrap 단계의 총량은 최대 약 200,000개 의사결정이다.

이 숫자는 보장값이 아니라 현재 구조와 관측값에 근거한 사전 예측이다. 1대1 공격·garbage·B2B 판단은 solo teacher가 가르치지 않으므로 이 학습만으로 1대1 기본 성능을 주장할 수 없다.

## 왜 수백만 게임이 필요하지 않은가

현재 교사는 10개 afterstate feature의 결정론적 선형 함수다. 정규화 전 후보 feature를 `x`, 교사 weight를 `w`라고 하면 점수는 다음과 같다.

```text
q(x) = w^T x
```

학습 target은 같은 상태의 모든 후보에 대한 다음 분포다.

```text
p_teacher(j | s) = softmax(q(x_j) / (c * tau))
```

ReLU는 `x = ReLU(x) - ReLU(-x)`로 선형 함수를 표현할 수 있다. 10개 입력에 대해 첫 hidden width 64는 교사 선형 함수를 표현하기에 충분하므로, 2,817개 parameter의 모델 용량이 병목일 가능성은 낮다. 노이즈 없는 선형 회귀만 생각하면 11개 이상의 독립적인 feature 방향으로도 weight를 식별할 수 있지만, 실제 Tetris에서는 다음 이유로 훨씬 많은 상태가 필요하다.

- 연속된 board 상태끼리 강하게 상관된다.
- 낮은 stack의 쉬운 상태가 과대표집될 수 있다.
- 후보 점수 차이가 작은 상태에서는 작은 근사 오차도 선택을 바꾼다.
- 학생이 한 번 다른 수를 두면 이후 상태 분포가 교사 trajectory에서 벗어난다.

따라서 parameter 수에 단순 배수를 곱하는 대신, 유효 feature 차원과 sequential distribution shift를 함께 고려해야 한다.

## 표본 복잡도 근사

교사의 유효 선형 차원을 bias 포함 `d_eff = 11`, 신뢰 실패 확률을 `delta = 0.05`로 놓고 다음을 보수적인 크기 비교 지표로 사용한다.

```text
epsilon_proxy(N_eff) = sqrt((d_eff + ln(1 / delta)) / N_eff)
```

이 값은 MLP의 엄밀한 PAC upper bound나 실제 top-1 오차율이 아니다. 표본 수가 늘 때 선형 교사 근사의 불확실성이 어느 정도 비율로 줄어드는지 보는 order estimate다. 연속 상태 상관을 반영한 보수 열에서는 전체 의사결정의 25%만 독립 표본과 비슷하다고 가정했다.

| 교사 의사결정 수 | 독립 표본 가정 | 유효 표본 25% 가정 | 판단 |
|---:|---:|---:|---|
| 10,000 | 3.74% | 7.48% | 실행·손실 검증용, 기본 모델 판정에는 작음 |
| 30,000 | 2.16% | 4.32% | 최소 학습 규모 |
| 100,000 | 1.18% | 2.37% | 첫 권장 규모 |
| 300,000 | 0.68% | 1.37% | 동일 분포에서는 수익 체감 시작 |

오차가 대략 `1 / sqrt(N)`로만 감소한다면 오차 proxy를 절반으로 줄이는 데 데이터가 네 배 필요하다. 그래서 100,000개 이후에는 데이터의 양보다 학습자가 실제로 만드는 실패 상태를 포함시키는 것이 더 중요하다.

## sequential error가 중요한 이유

일반적인 behavior cloning은 교사 상태 분포에서 학습하지만, 실행 중에는 학생이 만든 상태를 방문한다. DAgger 논문은 교사 분포에서의 작은 분류 오차가 horizon `T` 동안 최악의 경우 `T^2 * epsilon` 규모로 누적될 수 있음을 설명하고, 학습자 방문 상태를 다시 모으는 방법으로 이를 완화한다.

독립 오차라는 낙관적 가정에서도 한 수 오차율이 1%라면 1,000수 동안 한 번도 다른 선택을 하지 않을 확률은 다음과 같다.

```text
(1 - 0.01)^1000 ≈ 0.000043
```

따라서 “교사와 모든 수가 동일한 게임”을 목표로 삼으면 데이터가 얼마여도 비현실적이다. 대신 다른 수를 골라도 교사 점수가 같은지, 점수 손실이 작고 이후 상태를 복구하는지를 측정해야 한다.

## 현재 smoke가 알려 준 것

현재 512-decision smoke에는 상태당 평균 68.60개 후보가 있다. 후보 1위와 2위의 교사 점수 차이는 다음과 같았다.

- 정확한 0점 동점: 274 / 512 = 53.515625%
- 중앙값: 0 milli-score
- 평균: 3,321.4140625 milli-score
- 90 percentile: 9,822 milli-score

즉 절반이 넘는 상태에서 여러 행동이 교사 기준으로 똑같이 좋다. 기존 validation top-1 59.375%는 deterministic tie-break index까지 맞췄는지를 세므로 실제 교사 모방 품질보다 엄격하며, 단독 중단 기준으로 사용하면 안 된다. 반대로 평균 teacher regret 1,961.234375 milli-score도 작은 smoke의 분포에만 해당하므로 100,000개 학습 결과를 예측하는 외삽값으로 쓰지 않는다.

## 기본 모델 합격 gate

100,000개 학습 후 다음 조건을 모두 확인한다. 후보 동점을 분리할 수 있도록 trainer/evaluator 지표를 보완해야 한다.

1. **Tie-aware optimal rate ≥ 97%**: 모델이 고른 후보의 교사 점수가 그 상태의 최대 교사 점수와 같음.
2. **Positive-margin agreement ≥ 95%**: 교사 1위가 유일한 상태에서 같은 최적 후보를 선택함.
3. **Mean normalized regret ≤ 5%**: 평균 regret이 validation의 양수 top-two margin 평균의 5% 이하.
4. **Held-out closed-loop survival**: 교사가 1,000수까지 생존한 최소 500개 미사용 seed 중 모델도 95% 이상에서 1,000수 생존.
5. **No seed leakage**: 같은 match/seed의 decision이 train과 validation에 동시에 들어가지 않음.

이 gate는 teacher imitation의 기본 성능을 뜻한다. 1대1 승률 gate는 fixed-cadence battle adapter와 versus teacher가 생긴 뒤 별도로 정의한다.

## 권장 실행 순서

### 1. 규모 benchmark

먼저 10,000개를 생성해 decision/s, candidate/s, gzip byte/decision, epoch time을 측정한다. 현재 512개 shard의 압축 크기를 단순 비례하면 임시 저장 공간은 다음 정도다.

| 의사결정 수 | 예상 임시 gzip 크기 |
|---:|---:|
| 10,000 | 약 16.1 MiB |
| 100,000 | 약 160.9 MiB |
| 200,000 | 약 321.8 MiB |
| 300,000 | 약 482.7 MiB |

실제 wall time은 측정 전에는 추정하지 않는다. records는 checkpoint 로드 검증과 tie-aware offline 평가 후 삭제한다.

### 2. 첫 본학습

```text
decisions: 100,000
batch_decisions: 32
epochs: 5 maximum
validation: whole match/seed split, approximately 20%
selection: minimum validation normalized regret
```

5 epoch 전에 validation regret 개선이 두 번 연속 미미하면 조기 종료한다. 현재 trainer에는 자동 early stopping과 tie-aware metric이 없으므로 본학습 전에 구현하는 것이 좋다.

### 3. 실패 원인별 다음 행동

| 결과 | 다음 행동 |
|---|---|
| train/validation 모두 낮음 | optimizer, normalization, temperature 또는 metric 구현 점검 |
| train 높고 validation 낮음 | seed·stack 높이 다양화, 정규화와 과적합 점검 |
| offline gate 통과, closed-loop 실패 | 데이터만 반복하지 말고 learner-state aggregation 실행 |
| solo closed-loop 통과 | checkpoint 보존, 임시 records 삭제, 1대1 teacher/adapter 단계로 이동 |

### 4. Dataset aggregation

1. 학생 policy로 상태를 방문한다.
2. 각 방문 상태의 모든 합법 후보를 기존 교사가 평가한다.
3. 50,000개를 기존 dataset에 합쳐 재학습한다.
4. 이를 두 번까지 반복하되 held-out closed-loop 개선이 1 percentage point 미만이면 중단한다.

이 단계가 끝나면 총 약 200,000개 의사결정으로 teacher distribution과 learner failure distribution을 함께 다루게 된다.

## 1대1에 대한 제한

Thiery와 Scherrer의 Tetris controller 검토는 feature 설계, weight 최적화, 구현 세부 차이와 큰 성능 분산을 강조한다. 현재 solo teacher는 stack 안정화 feature만 가지므로 garbage timing, spike, cancellation, B2B/Surge 유지, 상대 위험도는 학습할 수 없다. 따라서 위 규모를 아무리 늘려도 강한 1대1 전략이 자동으로 생기지 않는다.

이 checkpoint는 다음 단계의 초기 표현과 안전한 stack prior로만 사용한다. 1대1에서는 별도의 versus feature, 다양한 style teacher, fixed-cadence 대전 평가와 terminal-objective self-play RL이 필요하다.

## 근거 자료

- Ross, Gordon, Bagnell, 2011, [A Reduction of Imitation Learning and Structured Prediction to No-Regret Online Learning](https://proceedings.mlr.press/v15/ross11a.html): behavior cloning의 sequential distribution shift와 DAgger 근거.
- Foster, Block, Misra, 2024, [Is Behavior Cloning All You Need? Understanding Horizon in Imitation Learning](https://proceedings.neurips.cc/paper_files/paper/2024/hash/da84e39ae51fd26bb5110d9659c06e13-Abstract-Conference.html): log-loss behavior cloning의 horizon·policy complexity 조건에 대한 최신 이론 보완.
- Thiery, Scherrer, 2009, [Building Controllers for Tetris](https://doi.org/10.3233/ICG-2009-32102): feature 기반 Tetris controller와 평가 분산·구현 민감도.
- Szita, Lorincz, 2006, [Learning Tetris Using the Noisy Cross-Entropy Method](https://doi.org/10.1162/neco.2006.18.12.2936): afterstate feature의 선형 점수와 weight 최적화 계보.

자료 확인일: `2026-08-25`
