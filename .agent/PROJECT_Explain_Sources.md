# Research Sources and Evidence Ledger

Accessed: 2026-08-24
Use: researched design; current rule literals still require pinned replay/config fixtures. The claim-level ledger is `research/SOURCE_LEDGER.md`.

## Primary and near-primary TETR.IO sources

- TETR.IO official patch notes: https://tetr.io/about/patchnotes/
  Establishes dated upstream changes, including Season 2 (Beta 1.2.0), Beta 1.3.0 garbage-clear bonus, and Beta 1.5.0 All-Mini+/Clutch Clear changes. Latest displayed version at access time is BETA 1.7.8 (2026-04-01).
- TETR.IO official FAQ mechanics source: https://github.com/tetrio/faq/blob/main/mechanics.html
  Public handling explanations including DAS/ARR/DCD/SDF; exact current frame ordering still requires fixtures.
- TETR.IO API documentation: https://tetr.io/about/api/
  Public service API scope; not treated as an engine specification.
- TETR.IO terms: https://tetr.io/about/terms/
  Confirms the online service boundary. This project does not automate the service.
- TETR.IO file format specs: https://github.com/tetrio/tetrio-format-specs
  The official repository currently specifies RSD audio data but no `.ttr`/`.ttrm` replay schema. The project therefore requires a user-owned sample before implementing a version-pinned adapter.
- TETR.IO issue #608: https://github.com/tetrio/issues/issues/608
  Historical feature request mentioning raw `.ttrm` replay downloads. It is not a current format/API specification and does not authorize internal API access.
- TETR.IO Wiki mechanics: https://tetrio.wiki.gg/wiki/Mechanics
  Current SRS+/SRS-X, combo/B2B, spins and attack explanations; secondary/community-maintained even when citing maintainer material.
- TETRA LEAGUE page: https://tetrio.wiki.gg/wiki/TETRA_LEAGUE
  Season boundaries and Season 2 summary.
- Independent TetrisWiki TETR.IO overview: https://tetris.wiki/Tetr.io
  Cross-check for mode differences, especially QUICK PLAY.

## Implementations, protocols and experience reports

- Cold Clear: https://github.com/MinusKelvin/cold-clear
  Archived modern-versus Rust bot with battle, libtetris, optimizer, opening-book and TBP components. Architectural baseline, not TETR.IO rule authority.
- Tetris Bot Protocol: https://github.com/tetris-bot-protocol/tbp-spec
  Common frontend/bot interface and rationale; used to avoid a proprietary live-game adapter.
- TETR.IO bot protocol notes: https://github.com/lemoncove/tetrio-bot-docs
  Room options and piece RNG observations. Its README pins the documentation to client 6.2.0 (2022-03-16), so it is historical schema evidence only; values remain `OBSERVED` until current fixtures reproduce them.
- tetris-analyzes: https://github.com/EdamAme-x/tetris-analyzes
  - 2026-08-24에는 commit `712dc10`의 extractor를 container에서 실행해 2026-08-10 client asset을 재추출했다. 2026-05 snapshot과 TL option 31개 및 firepower 결과가 동일했고, current asset에서 53개 clear/combo/B2B/All-Clear case를 생성했다. firepower snapshot SHA-256은 `b92d2446e42752a8ba86d873696a83cee0d99223d4bdafc1355a22cabbb3206b`다. reference replay가 없으므로 `OBSERVED`로만 사용한다.
  Independent current-client extraction and freshness-check idea. Low-adoption implementation candidate, not rule authority.
- Fan attack calculator: https://github.com/skysomorphic/tetrio-attack-calculator
  Documents extrapolated older formulas and uncertainty; useful specifically as a warning not to copy formulas across versions.
- Reddit garbage discussion (2024-10-30): https://www.reddit.com/r/Tetris/comments/1gfo4ss/how_does_tetrio_garbage_work/
  Player reports on transit/cancellation/passthrough changes. Edge-case discovery only.
- Reddit Jstris/TETR.IO comparison (2021-09-25): https://www.reddit.com/r/Tetris/comments/pv32r6/are_jstris_and_tetrio_different/
  Historical player observations on combo and passthrough; demonstrates version drift.
- Reddit Tetris AI experience: https://www.reddit.com/r/Tetris/comments/udnowg/i_pit_my_old_tetris_bot_against_my_new_tetris_bot/
  Practitioner report on heuristic pruning and deeper lookahead; hypothesis source, not benchmark evidence.

## Tetris learning research

- Szita, I. and Lorincz, A. (2006), “Learning Tetris Using the Noisy Cross-Entropy Method,” Neural Computation 18(12), DOI 10.1162/neco.2006.18.12.2936. Author/record: https://www.cs.utexas.edu/~shivaram/readings/b2hd-SzitaLorincz2006.html
  Evidence for compact features and noisy cross-entropy as a serious non-CNN baseline.
- Thiery, C. and Scherrer, B. (2009), “Improvements on Learning Tetris with Cross Entropy,” International Computer Games Association Journal, DOI 10.3233/ICG-2009-32104. https://journals.sagepub.com/doi/pdf/10.3233/ICG-2009-32104
  Feature-set and cross-entropy improvements; BCTS lineage.
- Scherrer et al. (2015), “Approximate Modified Policy Iteration and its Application to the Game of Tetris,” JMLR 16(49):1629–1676. https://jmlr.org/papers/v16/scherrer15a.html
  Sample-efficient classification-based policy iteration and formal approximation analysis.
- Algorta and Simsek (2019), “The Game of Tetris in Machine Learning,” arXiv:1905.01652. https://arxiv.org/abs/1905.01652
  Historical survey and open challenges; prevents treating a single modern architecture as settled.
- Chen et al. (2026), “Bitboard version of Tetris AI,” arXiv:2603.26765. https://arxiv.org/abs/2603.26765
  Reports 53x simulator speedup, afterstate evaluation, and buffer-optimized PPO. Recent preprint; results must be independently reproduced and its solo/random-piece setup does not establish versus superiority.
- Ng, Harada and Russell (1999), “Policy Invariance under Reward Transformations,” ICML. https://ai.stanford.edu/~ang/papers/shaping-icml99.pdf
  Theoretical basis and necessity result for potential-based reward shaping in MDPs.
- Lu, Schwartz and Givigi (2014), “Policy Invariance under Reward Transformations for General-Sum Stochastic Games.” https://arxiv.org/abs/1401.3907
  Direct theoretical basis for Nash-equilibrium invariance under potential-based shaping in stochastic games.
- Devlin and Kudenko (2011), “Theoretical Considerations of Potential-Based Reward Shaping for Multi-Agent Systems.” https://www.ifaamas.org/Proceedings/aamas2011/papers/D1_G45.pdf
  Multi-agent shaping assumptions and cautions.
- OpenSpiel (Lanctot et al., 2019): https://arxiv.org/abs/1908.09453
  Multi-agent evaluation and self-play framework reference.
- PSRO (Lanctot et al., 2017): https://mlanctot.info/files/papers/nips17-psro.pdf
  Policy-set, empirical-payoff and meta-strategy reference for nonstationarity/cyclic policies; full payoff growth is too costly, so use a bounded adaptation.
- Stanford CS224R Tetris multi-agent project (2025): https://cs224r.stanford.edu/projects/pdfs/224R_Paper__1_.pdf
  A custom-environment student report used only as an imitation/multi-agent hypothesis, not as proof for this ruleset.
- Zhang, Cai and Nebel (2010), “Playing Tetris Using Learning by Imitation”: https://www.eurosis.org/cms/files/proceedings_full/GAMEON2010.deel1_2.11.10.rdo.pdf
  Direct Tetris placement-classification precedent; old non-TL environment, so feasibility evidence only.
- DAgger (Ross, Gordon and Bagnell, AISTATS 2011): https://proceedings.mlr.press/v15/ross11a.html
  Basis for relabeling learner-visited states to control sequential covariate shift.
- Deep Q-learning from Demonstrations (Hester et al., AAAI 2018): https://ojs.aaai.org/index.php/AAAI/article/view/11757
  Demonstration plus TD-learning precedent; its fixed-action DQN is not copied directly.
- Imitation Learning by Estimating Expertise of Demonstrators (Beliaev et al., ICML 2022): https://proceedings.mlr.press/v162/beliaev22a.html
  Warning and method reference for mixed-quality teachers.
- Expert Iteration (Anthony, Tian and Barber): https://arxiv.org/abs/1705.08439
  Search-teacher/apprentice improvement loop reference.

## Evidence cautions

- TETR.IO wiki, GitHub observations, and Reddit posts are not official engine source code.
- The current target is All-Mini+; All-Mini alone describes an earlier Season 2 state.
- Season 1 attack tables/formulas are not valid evidence for Season 2 without a current fixture.
- Solo Tetris scores do not predict 1v1 strength; all learning claims must be re-evaluated in the pinned versus arena.
- The 2026 bitboard work is recent and reports different board/generator tasks; it motivates experiments, not a predetermined model.
