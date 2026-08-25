# PROJECT Explain: Imitation Evaluation

## 확정 설계

- offline 평가는 validation decision의 모든 후보를 실제 PyTorch checkpoint로 다시 채점하며, 동일 teacher score 후보를 모두 최적으로 인정한다.
- closed-loop 평가는 미사용 seed에서 모델 선택을 실제 권위 Rust 엔진에 되먹임하여 1,000수 생존율을 측정한다.
- 모델 직렬화 형식은 기존 `model.pt` 하나다. JSON model export는 만들지 않는다.
- Python은 checkpoint 로드·정규화·추론을, Rust `arena::SoloBatch`는 후보 생성·상태 전이를 담당한다.
- PyO3 bridge는 feature byte buffer와 offset, 선택 index만 전달한다. 권위 상태를 Python에 복제하지 않는다.
- 승격기는 두 보고서의 checkpoint SHA-256, dataset ID, engine revision과 gate를 확인한 뒤 보고서를 최종 checkpoint metadata에 내장한다.

## 승격 기준

```text
tie-aware optimal rate >= 0.97
positive-margin agreement >= 0.95
mean normalized regret <= 0.05
500 unseen seeds의 1,000수 생존율 >= 0.95
```

구체적인 실행 명령과 실패 대응은 `Explanation/Imitation_Model_Evaluation_and_Promotion.md`에 둔다.
