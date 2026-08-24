# 출처 및 주장 원장

확인일: `2026-08-24`

원칙: 출처가 설명하는 범위를 넘어 현재 mechanics나 성능을 추론하지 않는다.

## A. TETR.IO mechanics

| ID | 출처 | 유형·시점 | 이 프로젝트에서 뒷받침하는 내용 | 상태·제한 |
|---|---|---|---|---|
| T01 | [공식 patch notes](https://tetr.io/about/patchnotes/) | 공식 변경 기록, 최신 표시 BETA 1.7.8 `2026-04-01` | target version, Season 2, B2B Charging/Surge, opener, All Clear, special +1, All-Mini+, Clutch Clear, bugfix 이력 | 변경 사실은 `CONFIRMED`; 전체 현재 상수표는 아님 |
| T02 | [공식 FAQ mechanics source](https://github.com/tetrio/faq/blob/main/mechanics.html) | 공식 GitHub FAQ | DAS/ARR/DCD/SDF 설명과 공개 handling 의미 | 현재 client의 모든 frame-order literal을 보장하지 않음 |
| T03 | [TETR.IO Wiki Mechanics](https://tetrio.wiki.gg/wiki/Mechanics) | 최신 community-maintained 2차 자료 | 10×40, SRS+, 20G, combo rounding, B2B/Surge, opener, garbage 개념 | `OBSERVED`; fixture 필요 |
| T04 | [TETR.IO Wiki Spins](https://tetrio.wiki.gg/wiki/Spins) | 2차 자료 | immobility, last rotation, T corner 판정과 All-Mini+ 설명 | edge case literal은 fixture 필요 |
| T05 | [TETR.IO Wiki TETRA LEAGUE](https://tetrio.wiki.gg/wiki/TETRA_LEAGUE) | 2차 자료 | TL/Season 구분과 multiplayer 맥락 | 정확한 현행 room option의 1차 근거가 아님 |
| T06 | [TetrisWiki TETR.IO](https://tetris.wiki/Tetr.io) | 독립 2차 자료 | mode 차이, 기본 기능 교차 확인 | 공식 규칙표가 아님 |
| T07 | [historical Room Config](https://github.com/lemoncove/tetrio-bot-docs/blob/master/Room_Config.md) | 비공식 protocol 문서, README상 client 6.2.0/2022-03-16 | room option field 목록과 과거 의미 | 현재 기본값에 사용 금지; `OBSERVED/HISTORICAL` |
| T08 | [historical Piece RNG](https://github.com/lemoncove/tetrio-bot-docs/blob/master/Piece_RNG.md) | 비공식 protocol 문서, 2022 | 과거 MINSTD/Fisher-Yates/7-bag 관찰 | 현행 RNG로 확정 금지 |
| T09 | [TETR.IO SRS+ issue #506](https://github.com/tetrio/issues/issues/506) | GitHub issue와 community/maintainer-adjacent 논의 | SRS+ table 차이를 검증할 사례 | 현재 textual specification 아님 |
| T10 | [Cold Clear](https://github.com/MinusKelvin/cold-clear) | Rust 공개 봇, 2024-01-22 archived | bitboard/battle/search/opening/TBP 구조 참고 | 현재 TL rule authority 아님 |
| T11 | [Tetris Bot Protocol](https://github.com/tetris-bot-protocol/tbp-spec) | 공개 protocol specification | local engine-bot adapter 경계 | live service 연결 허가가 아님 |
| T12 | [tetris-analyzes](https://github.com/EdamAme-x/tetris-analyzes) | 최근 독립 extractor, 낮은 adoption | client-derived kick/spin/firepower snapshot과 freshness-check 아이디어 | 구현 후보; 독립 검증 전 authority 아님 |
| T13 | [Season 2 opener double-cancel 논의](https://www.reddit.com/r/Tetris/comments/1hjvuig) | Reddit 경험 기록 | opener 경계 fixture 후보 | 가설 생성용만 사용 |
| T14 | [Season 2 Surge/downstack 논의](https://www.reddit.com/r/Tetris/comments/1ogo885) | Reddit 경험 기록 | Surge가 전략에 미치는 체감과 adversarial test style | 정량·규칙 확정 근거 아님 |

## B. 학습·보상·평가

| ID | 출처 | 유형·시점 | 이 프로젝트에서 뒷받침하는 내용 | 상태·제한 |
|---|---|---|---|---|
| R01 | [Scherrer et al., JMLR 2015](https://jmlr.org/papers/v16/scherrer15a.html) | peer-reviewed | compact feature와 approximate policy iteration의 sample-efficient Tetris 기준선 | solo Tetris; versus 직접 증명 아님 |
| R02 | [Algorta & Şimşek survey, 2019](https://arxiv.org/abs/1905.01652) | survey/preprint | afterstate/placement formulation, 구현 차이와 고분산 평가 위험 | 최신 TL mechanics 자료 아님 |
| R03 | [Chen et al., 2026](https://arxiv.org/abs/2603.26765) | 최근 preprint | bitboard throughput과 afterstate actor 후보 | solo 10×10·다른 generator; 보고 수치를 본 프로젝트 성능 약속으로 사용 금지 |
| R04 | [Ng, Harada & Russell, 1999](https://ai.stanford.edu/~ang/papers/shaping-icml99.pdf) | 고전 이론 논문 | MDP potential-based shaping의 policy invariance | 2인 game에는 R05를 직접 사용 |
| R05 | [Lu, Schwartz & Givigi, 2014](https://arxiv.org/abs/1401.3907) | stochastic-game 이론 | potential shaping 아래 Nash equilibrium invariance | theorem 가정을 구현 test로 확인해야 함 |
| R06 | [Devlin & Kudenko, AAMAS 2011](https://www.ifaamas.org/Proceedings/aamas2011/papers/D1_G45.pdf) | conference paper | multi-agent PBRS의 이론적 조건과 주의점 | approximation의 속도 향상 보장 아님 |
| R07 | [Devlin & Kudenko, AAMAS 2012](https://www.ifaamas.org/Proceedings/aamas2012/papers/2C_3.pdf) | conference paper | dynamic potential shaping 확장 | 초기 버전에는 고정 potential을 우선 사용 |
| R08 | [Lanctot et al., PSRO 2017](https://mlanctot.info/files/papers/nips17-psro.pdf) | peer-reviewed | policy population, empirical payoff, meta-strategy, best-response oracle | full PSRO는 payoff 비용이 커 축소 적용 |
| R09 | [OpenSpiel, 2019](https://arxiv.org/abs/1908.09453) | research framework paper | game/RL/search 평가와 learning-dynamics 도구 | TETR.IO implementation은 제공하지 않음 |
| R10 | [AlphaStar, Nature 2019](https://doi.org/10.1038/s41586-019-1724-z) | peer-reviewed | historical policies와 exploiter를 둔 league 개념 | 계산 규모와 결과를 이 프로젝트로 전이하지 않음 |
| R11 | [Stanford CS224R Tetris multi-agent project, 2025](https://cs224r.stanford.edu/projects/pdfs/224R_Paper__1_.pdf) | student report, non-peer-reviewed | multi-agent imitation/AIRL 탐색 가설 | custom rules, 좁은 baseline; 모델 선택 근거로 단독 사용 금지 |
| R12 | [Zhang, Cai & Nebel, Tetris Learning by Imitation, 2010](https://www.eurosis.org/cms/files/proceedings_full/GAMEON2010.deel1_2.11.10.rdo.pdf) | conference proceedings | Tetris placement imitation을 classification으로 구성하고 style을 학습할 가능성 | 오래된 별도 platform; current 1v1 성능 근거 아님 |
| R13 | [Ross, Gordon & Bagnell, DAgger, 2011](https://proceedings.mlr.press/v15/ross11a.html) | peer-reviewed | sequential imitation의 covariate shift와 learner-state dataset aggregation | 자동 teacher 질의 비용과 반복 학습 비용을 예산화해야 함 |
| R14 | [Hester et al., DQfD, AAAI 2018](https://ojs.aaai.org/index.php/AAAI/article/view/11757) | peer-reviewed | supervised demonstration loss와 TD learning 결합이 초기 학습을 가속할 가능성 | fixed action DQN을 variable afterstate policy에 그대로 적용하지 않음 |
| R15 | [Beliaev et al., ICML 2022](https://proceedings.mlr.press/v162/beliaev22a.html) | peer-reviewed | 여러 demonstrator의 state별 expertise 차이를 무시하면 약한 행동을 흡수할 위험 | 초기에는 명시적 teacher rating/margin weighting을 우선 |
| R16 | [Anthony, Tian & Barber, Expert Iteration](https://arxiv.org/abs/1705.08439) | research paper | search expert와 apprentice policy를 반복 개선하는 구조 | Tetris 1v1에서 별도 재현 필요 |
| R17 | [AlphaStar, Nature 2019](https://doi.org/10.1038/s41586-019-1724-z) | peer-reviewed | supervised replay initialization 뒤 multi-agent RL을 수행한 대규모 사례 | human data·compute 규모를 전이하지 않고 순서만 참고 |

## C. 채택 규칙

- `T01`의 버전별 변경 사실과 current replay/config fixture가 충돌하면 먼저 target version과 fixture provenance를 재검토한다.
- Wiki·GitHub 구현·Reddit은 서로 많이 일치해도 공식 current fixture를 대신하지 않는다.
- 논문 성능 수치는 board 크기, generator, action space, terminal condition과 compute가 같은 경우에만 직접 비교한다.
- 모델·보상·self-play 방식은 출처의 명성보다 이 프로젝트의 고정 예산 paired experiment 결과로 최종 결정한다.
- demonstration dataset은 선택 action뿐 아니라 legal candidates, teacher score/margin, rules/engine/teacher hash를 보존하고 seed/match 단위로 분리한다.
- 모든 사용 자료는 license와 재배포 조건을 확인하며, 공개 코드를 복사할 때는 provenance와 license를 별도 기록한다.
