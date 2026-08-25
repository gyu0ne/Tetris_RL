# Execution Plan

Baseline date: 2026-08-24T15:05:44+09:00
Current state: declared learning-relevant solo/1v1 mechanics and the production-scale solo imitation pipeline are implemented; the current user-approved completion boundary is generation and verification of the long-run solo checkpoint, while all 1v1 learning implementation is deferred

Current scope freeze (2026-08-25): execute and validate the solo pipeline described in `PROJECT_Explain_Solo_Learning_Completion_Plan.md`. Phases 5–8 and any fixed-cadence 1v1 learning adapter remain future work until the user explicitly reopens that scope.

## Phase 0 — Evidence and specification freeze

Deliverables:

- Record current hardware/OS/container limits and target inference time budget.
- Collect user-owned TETR.IO custom/replay fixtures and their exported option objects for the pinned version.
- Convert the research ledger into `configs/rules/` records with value, unit, version, confidence and fixture.
- Convert the conformance matrix into test IDs and expected observations.

Exit gate: no required TETRA LEAGUE rule literal is silently guessed; unknowns are explicit and have an experiment to resolve them.

## Phase 1 — Repository and deterministic engine skeleton

- Create the Rust workspace, Python package, container workflow, CI checks, licenses/notices, and directory structure defined in `RULE.md`.
- Implement bitboard field, tetromino geometry, seedable RNG/7-bag, spawn/hold/queue, SRS+ rotations and reachable placement enumeration.
- Add unit, property, golden and fuzz tests; benchmark empty stepping, collision, clear and afterstate enumeration.

Progress: workspace/container, bitboard, geometry, observed Park–Miller MINSTD `ZLOSIJT` 7-bag, configurable spawn/hold/queue, SRS+/180 tables and geometric reachable-lock BFS are implemented. Integer microcell fall phase, lock/reset timing and an evidence-bearing target profile are present. Current-client literals are populated as `OBSERVED` and locally executable while external differential conformance blockers remain.

Exit gate: deterministic replay/state hashes across debug/release; geometry/RNG/movement conformance C1 passes.

## Phase 2 — Timing and unscored solo sandbox

- Implement per-frame input ordering, DAS/ARR/SDF, IRS/IHS, gravity, lock/reset, ARE/line-clear delay and top-out semantics.
- Emit score-free line/spin/perfect-clear events and expose a single-board survival/test sandbox. Do not implement 40 LINES, BLITZ or ZEN/custom scoring/goal profiles.
- Keep canonical differential tests headless; a visual replay player/viewer is not required for engine conformance.

Progress: the observed TL gravity schedule, integer microcell fall phase, spawn/kick fractional phases, exact lock/reset boundaries, upstream kick numbering, ordered frame-aligned hold actions, IRS/IHS, All-Mini+, Clutch, perfect clear and typed top-out are implemented. `FrameSession` connects normalization through next spawn across multiple pieces. TETRA LEAGUE does not enforce room handling, so the generic `PlayerHandlingProfile` remains a sandbox/input-adapter concern; primary afterstate learning does not depend on raw OS-event repetition. `replay-conformance` compares fall phase, last action and top-out reason.

Exit gate: timing boundary fixtures and solo full replays have zero unexplained divergence.

## Phase 3 — TETRA LEAGUE Season 2 versus

- Implement All-Mini+, base attack, combo, B2B Charging/Surge, opener-phase cancellation, garbage-clear bonus, packet transit/cancel/cap/messiness/insertion, Clutch Clear and round-terminal rules.
- Add deterministic two-player scheduling and explicit latency models.
- Differential-test minimized cases; retain the formal 10,000 randomized seeded policy for a final `Conformant` report without blocking manual/heuristic development.

Exit gate: conformance C2–C5 pass for the declared corpus. Publish coverage and remaining `UNCONFIRMED` cases.

Progress: `crates/versus` implements observed TL attacks, B2B/Surge packet ordering, perfect-clear and garbage-clear bonuses, ordered incoming packets, cancellation, opener 14, transit 20, combo blocking, cap 8, provenance, change-on-attack hole RNG, packet-depletion sample consumption and margin scaling. `BattleSession` performs simultaneous two-player lock/attack/tank/terminal resolution. `crates/replay-conformance` loads hash-bound normalized JSON v1 traces. `crates/manual-playground` now exposes the authoritative solo `FrameSession` for direct keyboard testing; pixel/UI parity is explicitly irrelevant. The profile remains externally unverified, but that label no longer blocks heuristic arena and exploratory imitation implementation.

## Phase 4 — Bot arena and strong baselines

- Implement bot protocol adapter, time/node budgets, reproducible tournaments and opponent snapshots.
- Add random, linear DT-feature, beam/MCTS and licensed external-protocol baselines.
- Establish strength/latency/throughput baselines and style clusters (opener, downstack, pressure, defense).

Exit gate: repeatable ratings with confidence intervals; no training system yet.

## Phase 4.5 — Heuristic demonstration bootstrap

- Generate versioned trajectories from diverse linear/search teachers and the frozen opponent pool. `OBSERVED` exploratory shards require explicit labeling/opt-in; release training remains conformance-gated.
- Store all legal afterstate scores/ranks, teacher margin/budget/style, rules/engine hashes, seeds and outcomes in sharded records.
- Train chosen-action BC, full-score/rank distillation and value initialization; then aggregate teacher labels on learner-visited states.
- Run the fixed final solo budget of at least 1M teacher decisions, three independent initializations and up to two 250k learner-state aggregation rounds before versus RL.

Progress: `crates/arena` implements hold-aware geometric candidates, ten integer features, a Dellacherie linear teacher, deterministic compressed full-score records and a native batched learner-state label bridge. `python/tetris_rl` validates provenance/integrity and trains a CPU `10→64→32→1` shared scorer with bounded shuffle, best-epoch restore, early stopping and three independent initializations. Actual PyTorch candidates are selected through paired offline/development closed-loop gates; a separate 20M-placement zero-top-out run gates promotion. Failure invokes up to two 250k learner-state aggregation rounds. `scripts/run-final-solo-bootstrap.ps1` executes the whole solo pipeline. The long run itself and fixed-cadence versus teacher/RL remain.

Exit gate: the imitation checkpoint improves held-out closed-loop strength under a fixed latency budget, has zero illegal actions, and beats both random initialization and chosen-only BC. Offline accuracy alone cannot pass the gate.

## Phase 5 — RL environment and reward verification

- Expose GIL-free batched afterstate environments and observation/action schemas.
- Implement terminal zero-sum reward and potential-shaping framework.
- Run reduced-game exact policy/Nash-equilibrium-invariance tests and per-feature bounds/symmetry tests.
- Pre-register linear, MLP, hybrid spatial and algorithm comparisons with fixed budgets.

Exit gate: reward theorem assumptions and tests pass; initial models cannot access hidden information or illegal actions.

## Phase 6 — Training ladder

- Train/evaluate linear scorer, small MLP, then hybrid encoder only if prior gates justify it.
- Compare PPO candidate with noisy cross-entropy/ES and the approved imitation initialization.
- Add historical opponent pool and exploit policies; promote checkpoints only on the frozen suite.
- Run reward feature ablations and paired statistical analysis.

Exit gate: selected architecture wins on held-out strength per wall-time/inference-budget, and every retained shaping feature has documented evidence.

## Phase 7 — Performance engineering

- Profile native simulation, enumeration, bridge, inference and optimizer.
- Apply measured optimizations: SIMD/bit tricks, transposition caching, batching, buffer reuse, async rollout and safe reduced precision.
- Verify that each optimization leaves golden replays and model evaluation unchanged within declared tolerance.

Exit gate: target steps/s and move latency are met on the recorded limited hardware without conformance regression.

## Phase 8 — Final validation and handoff

- Run the complete conformance corpus, fuzz/property suite, release build, tournament, ablations and reproducibility rerun.
- Update all `PROJECT_Explain_*` files with final constants, model, reward weights, benchmarks and limitations.
- Provide local play/arena/training commands and checkpoint/model cards.

Exit gate: another developer can reproduce the engine tests, train/evaluate a small run, and replay the published tournament from containers alone.

## Immediate next actions

1. Run `scripts/run-final-solo-bootstrap.ps1` from its clean committed revision.
2. Retain the promoted `checkpoints/solo-imitation-versus-bootstrap-v1/model.pt` and verify that it loads independently with matching dataset and engine metadata.
3. If the zero-top-out gate fails after both automatic learner-state aggregation rounds, stop and plan a solo feature/model/teacher redesign; do not lower the gate.
4. Do not implement fixed-cadence 1v1 learning, self-play or versus observations until the user explicitly reopens that scope.

## Key risks

- **Upstream opacity/drift:** pin version and store fixtures; never silently follow latest behavior.
- **Replay format/API changes:** use user-owned exports and an internal normalized schema.
- **False equivalence:** publish fixture coverage and confidence, not a blanket unsupported claim.
- **Reward hacking:** terminal objective, potential-only shaping, exact reduced-game checks and opponent-pool evaluation.
- **Self-play cycles:** historical/exploit opponents and held-out style clusters.
- **Compute exhaustion:** native batching, small-model-first successive halving, and hard wall-time budgets.
- **Legal/fair-play concerns:** local-only arena, original assets/naming, license review, no live-service automation.
