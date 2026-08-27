# PROJECT Explain: Solo Inference and Spectator

## 확정 설계

- 승격 checkpoint 관전은 별도 모델 export 없이 `LoadedScorer`가 실제 `model.pt`를 로드한다.
- 행동 단위는 학습과 같은 reachable locked afterstate다. 프레임별 이동 애니메이션은 모델 판단에 없으므로 구현하지 않는다.
- Rust `SoloBatch`가 권위 상태와 후보 열거·착지 적용을 소유하고 Python은 feature 정규화·PyTorch 채점·argmax만 담당한다.
- inference candidate는 feature, placement, hold flag만 보유한다. teacher score/checksum/path/event를 포함하는 labeled candidate는 learner-state aggregation에서만 생성한다.
- inference/labeled 경로는 동일 seed의 candidate order와 feature equality를 테스트로 고정한다.
- closed-loop 평가의 병렬 단위는 게임 seed shard다. 각 worker는 checkpoint를 독립 로드하고 결과는 seed 수로 가중 결합한다.
- 브라우저 관전자는 localhost 전용 Python 표준 HTTP 서버와 정적 HTML/CSS/JS로 구성하고, board mechanics를 복제하지 않는다.

## 인터페이스

- Rust/PyO3: `SoloBatch.candidates`, `labeled_candidates`, `step`, `snapshot`
- Python: `evaluate_checkpoint_parallel`, `SpectatorController`
- HTTP: `GET /api/state`, `POST /api/step`, `POST /api/reset`
- 운영: `docker compose up --build spectator`, `http://127.0.0.1:8788`

## 성능·동등성 경계

- worker 수와 thread 수는 wall time만 바꾸며 평가 seed와 모델 행동을 바꾸지 않는다.
- 프로세스 시작과 checkpoint 재로드 overhead 때문에 작은 smoke는 선형 가속을 기대하지 않는다.
- 관전자 raw score는 후보 집합 안의 상대 선택을 설명하는 진단값이며, 서로 다른 보드 사이의 절대 강도 비교값으로 사용하지 않는다.
- 기존 장기 승격 보고서가 모델 성능의 권위 자료이고 관전 화면은 수동 확인 도구다.
