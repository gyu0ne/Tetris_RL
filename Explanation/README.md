# Explanation 문서 목록

이 폴더는 구현된 엔진 기능, 모델, 학습·평가 파이프라인과 운영 절차를 주제별로 설명하는 한국어 기술 설명서 모음이다. 새 구성요소를 만들 때는 한 파일에 한 주제만 기록하고, 같은 구성요소가 바뀌면 기존 문서를 함께 갱신한다.

`.agent/PROJECT_Explain_*.md`는 설계 결정과 변경 이력을 보존하는 내부 문서이고, 이 폴더는 구현 결과를 독립적으로 이해하고 실행하기 위한 열람용 설명서다.

## 모델과 학습

- [솔로 모방학습 실행 절차](./Imitation_Learning_Runbook.md): 한 명령 자동 실행과 수동 생성·학습·검증·재학습·승격 절차
- [모방학습 모델 평가와 승격](./Imitation_Model_Evaluation_and_Promotion.md): offline·closed-loop 평가의 차이, 실제 `model.pt` 평가 구조와 실사용 체크포인트 생성 절차
- [장기 모방학습 규모 근거](./Imitation_Learning_Scale_Estimate.md): 100만 교사 결정·3개 초기화·최대 100 epoch·learner-state 추가·장기 생존 실행값의 근거
