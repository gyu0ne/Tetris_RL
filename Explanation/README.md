# Explanation 문서 목록

이 폴더는 구현된 엔진 기능, 모델, 학습·평가 파이프라인과 운영 절차를 주제별로 설명하는 한국어 기술 설명서 모음이다. 새 구성요소를 만들 때는 한 파일에 한 주제만 기록하고, 같은 구성요소가 바뀌면 기존 문서를 함께 갱신한다.

`.agent/PROJECT_Explain_*.md`는 설계 결정과 변경 이력을 보존하는 내부 문서이고, 이 폴더는 구현 결과를 독립적으로 이해하고 실행하기 위한 열람용 설명서다.

## 모델과 학습

- [솔로 모방학습 실행 절차](./Imitation_Learning_Runbook.md): 한 명령 자동 실행과 수동 생성·학습·검증·재학습·승격 절차
- [모방학습 모델 평가와 승격](./Imitation_Model_Evaluation_and_Promotion.md): offline·closed-loop 평가의 차이, 실제 `model.pt` 평가 구조와 실사용 체크포인트 생성 절차
- [장기 모방학습 규모 근거](./Imitation_Learning_Scale_Estimate.md): 100만 교사 결정·3개 초기화·최대 100 epoch·learner-state 추가·장기 생존 실행값의 근거
- [솔로 모델 관전자와 고속 평가](./Solo_Model_Spectator_and_Fast_Evaluation.md): 승격된 `model.pt`를 직접 보는 방법, 추론 전용 Rust 경로, seed 병렬 평가와 자원 프로필
- [1대1 자기대전 r2 설계와 실행](./Versus_Self_Play_R2_Design_and_Runbook.md): 장기 승패 할인, joint residual, 솔로 KL 보존, PFSP, 공격·안정성·방어 지표와 실행 절차
- [1대1 자기대전 r3 보상·크레딧](./Versus_Self_Play_R3_Reward_and_Credit.md): 전술 준비도 잠재함수, 안전성 제한 전술 커리큘럼, 긴 승패 trace와 실행·로그 판정
- [1대1 학습 성능 최적화와 보상 진단](./Versus_Training_Performance_and_Reward_Diagnostics.md): 후보 생성·PPO 병목 제거, Rayon/PyTorch 분리, terminal·potential shaping 로그 해석
- [1대1 자기대전 r1 기록](./Versus_Self_Play_Reinforcement_Learning.md): 기존 placement-level PPO와 entropy 실패·수정의 역사적 설명
- [사람 대 모델 로컬 대국](./Human_Versus_Model_Local_Battle.md): 실제 키보드 입력으로 현재 1대1 checkpoint와 겨루는 방법과 프레임/착지점 혼합 실행 구조
