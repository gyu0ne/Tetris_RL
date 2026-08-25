# 모방학습 모델 평가와 승격

작성일: `2026-08-25`

## Offline 평가

학습에 쓰지 않은 match seed의 모든 착지 후보를 실제 `model.pt`로 채점한다. 교사 점수가 같은 후보는 인덱스가 달라도 정답으로 인정한다. 주요 값은 `tie_aware_optimal_rate`, `positive_margin_agreement`, `mean_normalized_regret`다.

## Closed-loop 평가

Rust 권위 엔진이 합법 후보를 만들고 Python/PyTorch가 실제 `model.pt`로 선택한다. 선택 결과를 다시 Rust 상태에 적용하므로 모델 자신의 오류가 다음 상태에 누적된다. 모델 가중치를 JSON으로 변환하지 않는다.

후보 선택은 미사용 seed 256개 × 2,000수에서 수행한다. 최종 승격은 별도의 미사용 seed 2,000개 × 10,000수, 총 2,000만 placement에서 top-out 0회일 때만 가능하다.

## 실패의 의미

- Offline 실패: 교사 행동 자체를 충분히 근사하지 못했다. best epoch, learning rate 또는 모델 표현을 조정한다.
- Offline 통과·closed-loop 실패: 학습 분포 이탈이다. `tetris_rl.training.aggregate`로 learner-state 25만 개를 추가한다.
- 두 단계 통과: `tetris_rl.evaluation.promote`가 선택·평가 보고서를 최종 PyTorch 체크포인트에 내장한다.

전체 명령은 `Explanation/Imitation_Learning_Runbook.md`를 따른다.
