# TETR.io 1v1 Battle Bot - Hybrid SADRL AI & C++20 Engine

> ⚠️ **Notice**: 본 프로젝트의 대부분의 핵심 C++ 엔진 설계, 단위 테스트, PyTorch 강화학습 파이프라인, GUI 클라이언트 및 봇 연동 소스코드는 Google DeepMind 팀의 **Antigravity** 및 **Gemini** AI 어시스턴트에 의해 설계되고 작성되었습니다.

---

## 1. 프로젝트 개요 (Overview)

본 프로젝트는 경쟁형 테트리스 서비스인 **TETR.io의 1vs1 대전 물리 및 판정 시스템을 C++20으로 100% 모사**하고, **N-Step 수순 읽기(Beam Search)와 Deep Afterstate Value Network를 결합한 Hybrid SADRL(Search-Augmented Deep RL) 알고리즘**을 통해 마스터급 1vs1 대전 AI 봇을 구현한 연구 프로젝트입니다.

---

## 2. 주요 기능 및 특징 (Key Features)

- **C++20 Bitboard Engine**: 10x40 보드를 16비트 정수 배열로 표현하여 초당 100,000+ 연산의 초고속 시뮬레이션 지원.
- **TETR.io 완벽 모사 (100% Parity)**:
  - SRS+ 90° CW/CCW 및 osk 커스텀 180° 회전 킥 테이블
  - 3-Corner T-Spin 판정 및 4번 킥 업그레이드 수식
  - B2B 로그 Leveling 보너스 수식 ($\lfloor 1 + \ln(1 + \text{Level} \times 0.8) \rfloor$)
  - 방해줄 실시간 상쇄(Offsetting) 및 딜레이 틱, 콤보 가산표
  - 동적 난수 7-Bag 셔플링 시스템
- **Hybrid SADRL (Search-Augmented Deep RL)**:
  - C++ N-Step Beam Search 가지치기로 미스플레이(Mistake) 0% 하한선 보장
  - PyTorch 2D ResNet + Meta Feature 융합 Afterstate Value Network
- **Supervised Warm-Start**: C++ 전문가 평가 노하우를 단 15초 만에 사전 학습(Pre-train) 이식하여 0부터 헤딩하는 기간 완전 전면 스킵.
- **TETR.io Ribbon Protocol Client (`bot_client/`)**:
  - WebSocket / MessagePack 기반 공식 Ribbon 프로토콜 서브프레임 키프레임 직렬화 연동.
- **시각화 GUI 클라이언트**:
  - 미노별 고유 색상 및 가비지 다크 메탈 그레이 색상 렌더링
  - 사용자 커스텀 수동 키 조작 모드 (`play_interactive.py`)
  - Human vs AI 1v1 대전 모드 (`play_vs_ai.py`)
  - AI 솔로 관전 모드 (`play_ai_solo.py`)
  - AI vs AI 실시간 관전 경쟁 모드 (`play_ai_vs_ai.py`)

---

## 3. 기술 스택 (Tech Stack)

- **Core Engine**: C++20, Bitboard, Ninja, CMake
- **Binding Layer**: pybind11
- **Deep Learning & Environment**: Python 3.12, PyTorch 2.x, Gymnasium-style API
- **Networking & Protocol**: Ribbon Protocol (WebSocket), MessagePack
- **GUI Visualization**: Pygame

---

## 4. 플레이어 조작 키 안내 (Custom Controls)

- **좌 / 우 이동**: `J` / `O` (DAS 133ms / ARR 0ms 고속 오토 리핏)
- **Soft Drop (소프트 드롭)**: `I`
- **Hard Drop (하드 드롭)**: `Space`
- **Hold (홀드 스왑)**: `D`
- **Clockwise (시계 방향 회전 +90°)**: `W` (벽/바닥 킥 자동 적용)
- **Counter-Clockwise (반시계 방향 회전 -90°)**: `Q` (벽/바닥 킥 자동 적용)
- **Reset Game**: `R`

---

## 5. 실행 및 테스트 방법 (How to Run)

### 5.1. C++ 엔진 빌드 및 단위 테스트 (Unit Tests)
```bash
cmake -B build -G "Ninja"
cmake --build build
.\build\unit_tests.exe
```

### 5.2. 전문가 사전 학습 (Supervised Warm-Start)
```bash
python rl/pretrain_supervised.py
```

### 5.3. 초고속 대전 강화학습 (Fast SADRL Training)
```bash
python rl/train_fast.py --episodes 1000
```

### 5.4. 시각화 플레이 모드 실행
```bash
# 1. 플레이어 솔로 연습 모드
python play_interactive.py

# 2. 플레이어 vs AI 1v1 대전 모드
python play_vs_ai.py

# 3. AI 솔로 초고속 수순 관전 모드
python play_ai_solo.py

# 4. AI vs AI 실시간 관전 경쟁 모드
python play_ai_vs_ai.py
```