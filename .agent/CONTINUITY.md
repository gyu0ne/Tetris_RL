# Continuity

## [PLANS]

- 2026-08-24T14:18:08+09:00 [USER] Build a local TETR.IO-equivalent engine first, then a compute-efficient reinforcement-learning 1v1 bot; keep features separated and document major changes and settled designs.
- 2026-08-24T14:18:08+09:00 [CODE] Execute the gated roadmap in `PROJECT_Explain_Execution_Plan.md`: specification and fixtures, deterministic core, versus rules, conformance, baselines, RL/reward experiments, optimization, final evaluation.
- 2026-08-24T14:35:07+09:00 [USER] Consolidate and translate the initial design into one Korean HTML report optimized for personal browser viewing with a technical tone and A4 report layout.
- 2026-08-24T14:55:50+09:00 [USER] Supersede the report format request with one explanatory Korean Markdown file in the project root.
- 2026-08-24T15:22:13+09:00 [USER] Research and organize the mechanics and RL evidence; interpret “exactly same” as engine mechanics equality, excluding game-service systems when they do not affect learning.
- 2026-08-24T15:53:39+09:00 [USER] Begin engine implementation and research a pre-RL bootstrap that generates many heuristic Tetris records for imitation learning.
- 2026-08-24T16:05:13+09:00 [USER] Continue engine implementation; configure `https://github.com/gyu0ne/Tetris_RL` as the Git remote, strengthen ignore rules, and create local commits where possible.
- 2026-08-24T16:28:38+09:00 [USER] Continue implementation and research enough current evidence to fill as many empty mechanics values as possible.

## [DECISIONS]

- 2026-08-24T14:18:08+09:00 [CODE] Define “perfectly same” as observational conformance to a pinned upstream version because hidden server implementation details cannot be proven from public evidence. Tentative pin: TETR.IO BETA 1.7.8 with TETRA LEAGUE Season 2; exact exported settings remain UNCONFIRMED.
- 2026-08-24T14:18:08+09:00 [CODE] Use a deterministic Rust bitboard engine with versioned rules profiles and a narrow Python training bridge; this separates correctness-critical simulation from research iteration and reduces simulation cost.
- 2026-08-24T14:18:08+09:00 [CODE] Use reachable locked afterstates as the learning action set, then convert them to frame inputs with a deterministic planner. Final model architecture remains experiment-gated, not preselected as a CNN.
- 2026-08-24T14:18:08+09:00 [CODE] Preserve the zero-sum terminal objective; permit only bounded antisymmetric potential-based dense reward until mathematical and empirical verification is complete.
- 2026-08-24T14:18:08+09:00 [CODE] Prohibit automation against the live TETR.IO service; development and evaluation occur only in the local engine/arena.
- 2026-08-24T14:35:07+09:00 [CODE] Use a self-contained A4 HTML artifact with inline CSS/JS; the user-requested single-file constraint overrides Hallmark's separate `tokens.css` export convention. Use a static editorial Long Document/Grid-ultramarine system with no imagery or decorative motion.
- 2026-08-24T14:55:50+09:00 [CODE] Consolidate the six English governance/design/plan documents into `PROJECT_PLAN_KO.md`; preserve technical identifiers and all `UNCONFIRMED` qualifications. Keep the existing HTML because deletion was not requested.
- 2026-08-24T15:22:13+09:00 [CODE] Scope conformance by a learning-impact test: include anything changing legal reachability, observable transitions/RNG, attack/garbage, or round terminal; exclude accounts, rating, matchmaking, presentation, anti-cheat, and undisclosed infrastructure.
- 2026-08-24T15:22:13+09:00 [CODE] Correct the current target spin profile from launch-era Season 2 All-Mini to BETA 1.5.0 All-Mini+; retain all exact current TL timing/garbage/top-out literals as UNCONFIRMED until current fixtures exist.
- 2026-08-24T15:22:13+09:00 [CODE] Use stochastic-game potential-based reward invariance, not only the single-agent MDP theorem, as the theoretical gate; require player-swap antisymmetry, terminal normalization, reduced exact-game equilibrium checks, and paired ablations.
- 2026-08-24T15:53:39+09:00 [CODE] Implement Phase 1 as a generic deterministic kernel first; keep spawn, RNG, SRS+ and 180 literals explicitly OBSERVED/provisional until current target fixtures exist, and name reachability geometric until timing is modeled.
- 2026-08-24T15:53:39+09:00 [CODE] Add a pre-RL heuristic bootstrap: diverse linear/search teachers record all legal candidate scores and provenance, behavior cloning initializes policy/value, learner-state dataset aggregation reduces covariate shift, then terminal self-play RL removes the teacher ceiling.
- 2026-08-24T15:53:39+09:00 [CODE] Pin the container toolchain to Rust 1.89.0; no host packages were installed.
- 2026-08-24T16:05:13+09:00 [CODE] Separate target evidence from mechanics execution: `engine-core` consumes explicit generic timing parameters while `rules-tetrio` owns version/mode/provenance and refuses activation when any transition-critical required literal is absent.
- 2026-08-24T16:05:13+09:00 [CODE] Represent gravity as an integer rational accumulator and accept already ordered per-frame discrete actions; held-key interpretation and same-frame conflict resolution remain a versioned normalization layer pending fixtures.
- 2026-08-24T16:05:13+09:00 [ASSUMPTION] Configure the requested remote and create a local baseline commit, but do not push because the user requested remote setup and commits without explicitly requesting publication.
- 2026-08-24T16:09:06+09:00 [CODE] Normalize repository text to LF with `.gitattributes` while preserving CRLF for Windows batch files; this removes platform-dependent line-ending churn without changing source semantics.
- 2026-08-24T16:28:38+09:00 [CODE] Replace the binary incomplete/active profile state with two gates: complete client-derived `OBSERVED` profiles may execute local mechanics, while conformance remains blocked until fields are `CONFIRMED` by reference replay/config differential fixtures.
- 2026-08-24T16:28:38+09:00 [CODE] Treat TETRA LEAGUE room handling and effective player handling separately because the extracted profile has `room_handling=false`; inactive room ARR/DAS/SDF metadata must not become mode constants.
- 2026-08-24T16:28:38+09:00 [CODE] Define ordered-edge/DAS/ARR/DCD/sonic-drop normalization as a deterministic generic contract, not an exact TETR.IO stage-order claim; IRS/IHS and target ordering remain fixture-gated.
- 2026-08-24T16:28:38+09:00 [CODE] Keep the gravity schedule on one fixed rational denominator across all frames; reducing each frame independently would reinterpret the existing accumulator remainder when gravity changes and corrupt deterministic fall timing.

## [PROGRESS]

- 2026-08-24T14:18:08+09:00 [CODE] Created the initial governance, architecture, conformance, RL/reward, sources, and execution-plan documents under `.agent/`.
- 2026-08-24T14:18:08+09:00 [TOOL] Verified all 7 required documents and 361 lines: required sections/topics present, no missing files, placeholders, trailing whitespace, or missing final newlines.
- 2026-08-24T14:35:07+09:00 [CODE] Created `tetris_project_report_ko.html`, a 13-page Korean A4 report containing the translated project scope, architecture, conformance strategy, RL/reward design, roadmap, governance, risks, and source ledger.
- 2026-08-24T14:55:50+09:00 [CODE] Created root-level `PROJECT_PLAN_KO.md`, integrating the project rules, architecture, conformance strategy, RL/reward design, compute plan, phases, risks, sources, and completion gates in Korean.
- 2026-08-24T15:22:13+09:00 [CODE] Added `research/README.md`, `research/TETRIO_MECHANICS_RESEARCH_KO.md`, `research/RL_RESEARCH_KO.md`, and `research/SOURCE_LEDGER.md` to separate scope/mechanics, learning theory, and claim-level provenance.
- 2026-08-24T15:22:13+09:00 [CODE] Added `PROJECT_Explain_Mechanics_Scope.md` and updated RULE, architecture, conformance, RL/reward, sources, execution plan, and root Korean plan so the clarified mechanics boundary and research conclusions are consistent.
- 2026-08-24T15:24:02+09:00 [TOOL] Verified all 13 Markdown documents and 1,172 lines: all required files exist, final newlines are present, trailing whitespace is absent, and automated content checks confirm mechanics scope, All-Mini+, stochastic-game reward theory, and the historical-source warning.
- 2026-08-24T15:53:39+09:00 [CODE] Created the Cargo workspace, pinned Docker/Compose workflow, repository README/AGENTS, and `crates/engine-core` modules for board, pieces, RNG/bag, rotations, geometric reachability and queue/hold/game transitions.
- 2026-08-24T15:53:39+09:00 [CODE] Added engine and imitation design explanations plus `research/IMITATION_BOOTSTRAP_RESEARCH_KO.md`; updated RULE, architecture, execution plan, root plan, RL research and source ledger with the new pretraining gate and dataset provenance rules.
- 2026-08-24T15:53:39+09:00 [TOOL] Container verification passed: rustfmt check, clippy with `-D warnings`, 22 unit tests with 0 failures, and optimized release build.
- 2026-08-24T16:05:13+09:00 [CODE] Added `engine-core::timing` for rational gravity, ordered discrete inputs, hard drop, lock delay/reset cap, plus `rules-tetrio` for field-level evidence and target-profile activation validation; expanded `.gitignore` for build, ML, secret, editor, OS and temporary artifacts.
- 2026-08-24T16:05:13+09:00 [CODE] Updated README, Korean project plan, architecture/execution plans, RULE, and added `PROJECT_Explain_Timing_and_Rules_Profile.md` so implemented behavior and remaining conformance gaps are explicit.
- 2026-08-24T16:05:13+09:00 [TOOL] Container verification passed after the timing/profile changes: rustfmt check, clippy with `-D warnings`, 31 unit tests with 0 failures, and optimized release build.
- 2026-08-24T16:09:06+09:00 [CODE] Added repository-wide `.gitattributes` after initial staging exposed host-dependent LF-to-CRLF warnings; `.gitignore` continues to exclude generated Rust/Python/ML/profiling/secret/editor artifacts.
- 2026-08-24T16:10:33+09:00 [TOOL] Read-only `git ls-remote --symref` returned no refs for the requested GitHub URL; configured it as `origin`, renamed the local branch from `master` to `main`, and verified both fetch/push URLs without publishing data.
- 2026-08-24T16:10:33+09:00 [CODE] Created local root commit `f324bdc` (`feat: bootstrap deterministic Tetris engine`) containing 38 verified source, research, governance and container files; the working tree was clean and no push was performed.
- 2026-08-24T16:28:38+09:00 [TOOL] Ran the public `tetris-analyzes` freshness check in `oven/bun:1.3.10`; the stored 2026-05 asset was stale against current asset `63ab5c7c7.efa161fa8f91.20260810T191705`, then a fresh read-only research snapshot was generated.
- 2026-08-24T16:28:38+09:00 [TOOL] Compared all 31 extracted TL option fields between the 2026-05-04 and 2026-08-10 client assets: 0 fields changed. Recorded both asset IDs, extractor revision and snapshot SHA-256 values in the versioned observed TOML record.
- 2026-08-24T16:28:38+09:00 [CODE] Filled timing literals as `OBSERVED`: 60 Hz, ARE 0, line-clear ARE 0, initial gravity `1/50G`, increase `7/2000G/s` after 7200 frames, 20G cap, 30-frame lock, 15 resets, and move/rotation reset behavior.
- 2026-08-24T16:28:38+09:00 [CODE] Added `engine-core::handling`, exact-rational TL gravity scheduling, handling/profile tests, `configs/rules/` provenance records, and synchronized README, project plan, research, architecture, execution, rules and timing-design documents.
- 2026-08-24T16:28:38+09:00 [TOOL] The first expanded test run found one DCD boundary expectation off by one after applying both rotation and spawn pauses; the test was corrected to the documented generic pause contract and the rerun passed 33 engine-core plus 5 rules-tetrio tests.

## [DISCOVERIES]

- 2026-08-24T14:18:08+09:00 [TOOL] The workspace was empty and `.agent/CONTINUITY.md` was absent at task start.
- 2026-08-24T14:18:08+09:00 [TOOL] Persistent full code-graph indexing crashed; non-persistent fast indexing succeeded with 2 nodes and 1 edge, confirming no implementation code is present.
- 2026-08-24T14:18:08+09:00 [TOOL] `git diff --check` and `git status` were unavailable because the workspace is not initialized as a Git repository; no repository initialization was inferred or performed.
- 2026-08-24T14:18:08+09:00 [TOOL] Official patch notes establish Season 2 B2B Charging/Surge, All-Mini, 14-piece opener cancellation, and 5-line All Clear changes; exact current match option values still require exported replay/config fixtures.
- 2026-08-24T14:18:08+09:00 [TOOL] Research supports bitboard simulation and afterstate evaluation for sample/compute efficiency; potential-based reward shaping supplies a policy-invariance condition, but versus-specific feature effects still require proof on reduced games and controlled ablations.
- 2026-08-24T15:22:13+09:00 [TOOL] Official patch notes show the current displayed release is BETA 1.7.8 (2026-04-01) and BETA 1.5.0 changed multiplayer to All-Mini+ and reworked Clutch Clears; older protocol documentation is pinned to a 2022 client and cannot establish current literals.
- 2026-08-24T15:22:13+09:00 [TOOL] Public sources describe SRS+, handling, B2B Charging/Surge, opener cancellation and special garbage-clear behavior, but do not establish every current TL timing, garbage, RNG, top-out, or ordering literal; current user-owned replay/config fixtures remain required.
- 2026-08-24T15:22:13+09:00 [TOOL] Tetris research supports a linear-feature -> small afterstate MLP -> optional spatial encoder/search ladder. Recent bitboard/PPO results are solo preprint evidence only; PSRO/league ideas require a bounded pool under limited compute.
- 2026-08-24T15:53:39+09:00 [TOOL] Docker was initially stopped and no host Rust toolchain was present; after approved hidden Docker Desktop startup, daemon 29.6.2 built and verified the pinned Rust image without a host install.
- 2026-08-24T15:53:39+09:00 [TOOL] Fresh code-graph indexing found 399 nodes and 932 edges; the implemented call graph separates board/piece/RNG/rotation/reachability/game responsibilities inside `engine-core`.
- 2026-08-24T15:53:39+09:00 [TOOL] Tetris imitation, DAgger, DQfD, mixed-expertise imitation and Expert Iteration support demonstration bootstrap, but also show that chosen-only offline cloning can copy teacher weaknesses and fail on learner-induced states.
- 2026-08-24T16:05:13+09:00 [TOOL] Fresh non-persistent code-graph indexing found 548 nodes and 1,361 edges; it identifies `engine-core` as the leaf mechanics layer and `rules-tetrio` as a one-way entry/adapter dependency with no reverse coupling.
- 2026-08-24T16:05:13+09:00 [TOOL] The requested repository had no configured remote and every project file was untracked; no persistent `.codebase-memory` artifact existed, so the baseline commit can include all source/docs while generated caches remain excluded.
- 2026-08-24T16:28:38+09:00 [TOOL] The current client-derived TL timing values are stable across two asset versions and agree with the Wiki's 500 ms lock and move/rotation reset description, but no reference replay proves margin-boundary or same-frame execution order; confidence remains `OBSERVED`.
- 2026-08-24T16:28:38+09:00 [TOOL] Current TL exposes inactive room handling fallbacks ARR 2/DAS 10/SDF 6 while `room_handling=false`; using those numbers as every player's effective handling would change reachability and be incorrect.

## [OUTCOMES]

- 2026-08-24T14:18:08+09:00 [CODE] Initial pre-development analysis and document verification are complete. No engine or model implementation has begun; Phase 0 evidence capture is the next gate.
- 2026-08-24T14:55:50+09:00 [CODE] The requested Korean Markdown integration is complete. The planning status remains unchanged: implementation has not begun and Phase 0 evidence capture is next.
- 2026-08-24T15:22:13+09:00 [CODE] Initial evidence research and mechanics-scope consolidation are complete. Implementation remains gated on current replay/config capture, exact rules records, hardware budget declaration, and executable fixture manifests.
- 2026-08-24T15:53:39+09:00 [CODE] Supersede the previous implementation-not-started status: the first deterministic core is implemented and verified. Phase 1 target conformance is not complete; next work is versioned rules/fixtures followed by timing-aware input, lock and spin mechanics before versus and dataset generation.
- 2026-08-24T16:05:13+09:00 [CODE] The generic frame timing and evidence-bearing rules-profile foundation are implemented and verified. Target TL activation remains intentionally blocked by six unconfirmed timing literals; raw handling normalization, replay fixtures, spin/top-out and versus mechanics remain before heuristic dataset generation.
- 2026-08-24T16:28:38+09:00 [CODE] Supersede the six-empty-literal status: the observed TL timing profile is complete and locally executable, including gravity progression and generic handling normalization. It is not conformance-certified; player handling serialization, IRS/IHS, replay stage-order fixtures, spin/top-out and versus mechanics remain before demonstration generation.
