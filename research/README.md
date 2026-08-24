# 조사 자료 안내

최종 조사 기준 시각: `2026-08-24T15:05:44+09:00`

이 폴더는 TETR.IO mechanics 동등 엔진과 제한된 자원용 1 대 1 강화학습 봇을 설계하기 위한 근거를 모은다. 여기서 동등성 대상은 **학습 가능한 전략과 상태 전이에 영향을 주는 mechanics**다. 계정, 랭크 표시, 매치메이킹, UI, 음향, 외형, 안티치트, 공식 서버 운영은 포함하지 않는다.

## 문서 구성

- `TETRIO_MECHANICS_RESEARCH_KO.md`: 현재 확인된 mechanics, 제외 범위, 미확정 상수와 differential fixture 계획
- `RL_RESEARCH_KO.md`: 모델·알고리즘·보상·self-play·평가·연산 최적화 조사
- `IMITATION_BOOTSTRAP_RESEARCH_KO.md`: heuristic/search 기록 생성, behavior cloning, dataset aggregation과 RL 전환 설계
- `SOURCE_LEDGER.md`: 주장별 출처, 날짜, 근거 등급과 사용 제한

## 근거 상태

- `CONFIRMED`: 날짜가 있는 공식 변경 기록이나 동일 버전의 재현 가능한 기준 fixture로 확인함
- `OBSERVED`: 최신 Wiki, 공개 구현, 과거 프로토콜 문서 또는 복수의 관찰이 지지하지만 기준 fixture가 아직 없음
- `UNCONFIRMED`: 현재 기준 버전의 정확한 값이나 실행 순서를 공개 자료만으로 확정할 수 없음

`OBSERVED`와 `UNCONFIRMED` 값을 엔진 기본값으로 조용히 고정하지 않는다. 구현에 들어가기 전에 사용자가 소유한 replay/config export로 기준 fixture를 만들고, 근거와 hash를 함께 보관한다.

## 핵심 결론

1. 기준 프로필은 잠정적으로 `TETR.IO BETA 1.7.8 / TETRA LEAGUE Season 2`로 고정한다.
2. 현재 멀티플레이 기본 스핀 프로필은 Season 2 시작 당시의 `All-Mini`가 아니라 BETA 1.5.0에서 도입된 `All-Mini+`다.
3. 계정·등급·매치메이킹 같은 서비스 시스템은 제외하지만, 승패·동시 사망·clutch clear처럼 episode 종료와 보상에 영향을 주는 round mechanics는 포함한다.
4. 공개 문서만으로 현재 TL의 모든 timing·garbage·top-out 상수를 확정할 수 없다. 이 항목은 fixture 확보 전까지 `UNCONFIRMED`다.
5. 첫 모델은 CNN이 아니라 선형 afterstate 기준선과 작은 공유 MLP를 우선 비교한다. 공간 encoder는 같은 wall-time·추론 지연 예산에서 실제 대국 성능이 이길 때만 채택한다.
6. 기본 목적은 terminal 승패다. dense reward는 stochastic-game에서 정책/Nash 균형 불변 조건을 갖는 potential shaping으로만 도입하고, 학습 속도 향상 여부는 별도 실험으로 검증한다.
7. RL 전에 여러 heuristic/search teacher의 후보 전체 점수를 기록해 기본 모델을 pretrain한다. one-shot 행동 복제에 그치지 않고 learner-state dataset aggregation과 terminal RL fine-tuning으로 teacher ceiling과 분포 이탈을 다룬다.

## 사용 원칙

- 공식 patch note는 “언제 무엇이 바뀌었는가”의 우선 근거지만, 전체 현재 규칙표를 대신하지 않는다.
- Wiki와 커뮤니티 기록은 검증할 edge case를 찾는 데 사용하며 단독 확정 근거로 사용하지 않는다.
- 오래된 bot protocol 문서는 field 이름과 과거 관찰을 찾는 자료일 뿐 현재 TL 기본값의 근거가 아니다.
- Quick Play처럼 규칙이 다른 모드는 별도 profile로만 다루며 핵심 1 대 1 학습 환경에 섞지 않는다.
- 실제 TETR.IO 서비스에 봇을 연결하거나 입력을 주입하지 않는다.
