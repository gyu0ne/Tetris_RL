# 솔로 부트스트랩 학습 규모 근거

작성일: `2026-08-25`

현재 본학습은 짧은 10만 결정·5 epoch 계획을 폐기하고 다음 규모를 사용한다.

```text
서로 다른 교사 게임: 4,096
게임당 최대 결정: 250
최소 교사 결정: 1,000,000
독립 초기화: 3
최대 epoch: 100
최소 epoch: 20
early-stopping patience: 10
learner-state 추가량: 250,000 × 최대 2회
```

게임마다 실제 seed가 다르며, match seed 단위로 train/validation을 분리한다. 고정된 것은 재현 가능한 seed 목록을 만드는 공식뿐이다. 한 epoch는 약 100만 decision과 수천만 candidate afterstate를 모두 한 번 처리하므로 100 epoch는 상한일 뿐이다. 실제 선택값은 validation teacher regret가 가장 낮은 epoch다.

교사 trajectory만 반복하면 모델의 작은 오류가 만든 낯선 보드가 학습 데이터에 없을 수 있다. 따라서 최종 생존 평가에서 top-out이 하나라도 나오면 동일 데이터를 더 오래 반복하지 않고 모델 방문 상태를 교사가 다시 라벨링한다. 최대 총 학습량은 약 150만 decision이다.

최종 솔로 기준은 선택에 사용하지 않은 2,000개 seed에서 각 10,000 placement, 총 2,000만 placement의 top-out 0회다. 이 기준은 “절대”에 대한 수학적 증명이 아니라 1대1 self-play를 시작하기 위한 강한 경험적 생존 하한이다.
