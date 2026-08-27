# PROJECT Explain: Imitation Evaluation

## 확정 설계

- offline 평가는 validation decision의 모든 후보를 실제 PyTorch checkpoint로 다시 채점하며, 동일 teacher score 후보를 모두 최적으로 인정한다.
- closed-loop 평가는 미사용 seed에서 모델 선택을 실제 권위 Rust 엔진에 되먹임한다. 후보 선택은 256개 seed×2,000 placement, 최종 승격은 별도 2,000개 seed×10,000 placement를 사용한다.
- 모델 직렬화 형식은 기존 `model.pt` 하나다. JSON model export는 만들지 않는다.
- Python은 checkpoint 로드·정규화·추론을, Rust `arena::SoloBatch`는 후보 생성·상태 전이를 담당한다.
- PyO3 bridge는 추론 시 feature byte buffer와 offset, 선택 index만 전달한다. learner-state 집계용 별도 labeled 경로에서만 teacher-score buffer를 추가한다. 권위 상태를 Python에 복제하지 않는다.
- 서로 다른 초기화 seed로 학습한 세 checkpoint를 동일 validation과 동일 개발 seed 집합에서 비교하고, gate를 통과한 실제 `.pt` 하나를 byte-for-byte 복사한다.
- 승격기는 선택·offline·최종 closed-loop 보고서의 checkpoint SHA-256, dataset ID, engine revision과 gate를 확인한 뒤 보고서를 최종 checkpoint metadata에 내장한다.
- 최종 생존 실패는 gate 완화가 아니라 learner-state 250,000개 추가와 scratch 재학습으로 연결하며 최대 두 번만 수행한다.
- closed-loop의 독립 seed를 process shard로 나누고 worker별 PyTorch thread를 제한한다. shard 지표는 seed 가중 결합하며 worker 수는 평가 결과에 영향을 주지 않는다.

## 승격 기준

```text
tie-aware optimal rate >= 0.97
positive-margin agreement >= 0.95
mean normalized regret <= 0.05
후보 선택: 256 unseen seeds의 2,000수 생존율 = 1.0
최종 승격: 별도 2,000 unseen seeds의 10,000수 생존율 = 1.0
```

구체적인 실행 명령과 실패 대응은 `Explanation/Imitation_Model_Evaluation_and_Promotion.md`에 둔다.
