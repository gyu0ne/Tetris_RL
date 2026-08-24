# Execution Plan

Baseline date: 2026-08-24T15:05:44+09:00
Current state: Phase 1 deterministic core and Phase 2 timing foundation implemented; target fixtures/full handling/versus remain

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

Progress: workspace/container, bitboard, geometry, generic MINSTD 7-bag, configurable spawn/hold/queue, observed SRS+/180 tables and geometric reachable-lock BFS are implemented. Rational frame gravity, ordered discrete actions, configurable lock/reset timing, and an evidence-bearing target profile draft are also present. The pinned draft refuses activation while six required timing literals remain unknown. The workspace has 31 passing tests. Current spawn/RNG/kick/timing literals are not yet target-confirmed.

Exit gate: deterministic replay/state hashes across debug/release; geometry/RNG/movement conformance C1 passes.

## Phase 2 — Timing and solo profiles

- Implement per-frame input ordering, DAS/ARR/SDF, IRS/IHS, gravity, lock/reset, ARE/line-clear delay and top-out semantics.
- Add line/spin/perfect-clear detection and scoring profiles for 40 LINES, BLITZ and ZEN/custom.
- Create a minimal local CLI/replay viewer for diagnosis, kept outside engine state.

Progress: fixed-point/rational gravity, ordered normalized discrete actions, hard drop, and configurable lock/reset-cap transitions are implemented. Raw held-key normalization, exact target frame ordering, ARE, spin/top-out, and solo scoring remain.

Exit gate: timing boundary fixtures and solo full replays have zero unexplained divergence.

## Phase 3 — TETRA LEAGUE Season 2 versus

- Implement All-Mini+, base attack, combo, B2B Charging/Surge, opener-phase cancellation, garbage-clear bonus, packet transit/cancel/cap/messiness/insertion, Clutch Clear and round-terminal rules.
- Add deterministic two-player scheduling and explicit latency models.
- Differential-test minimized cases, then at least 10,000 randomized seeded cases.

Exit gate: conformance C2–C5 pass for the declared corpus. Publish coverage and remaining `UNCONFIRMED` cases.

## Phase 4 — Bot arena and strong baselines

- Implement bot protocol adapter, time/node budgets, reproducible tournaments and opponent snapshots.
- Add random, linear DT-feature, beam/MCTS and licensed external-protocol baselines.
- Establish strength/latency/throughput baselines and style clusters (opener, downstack, pressure, defense).

Exit gate: repeatable ratings with confidence intervals; no training system yet.

## Phase 4.5 — Heuristic demonstration bootstrap

- Generate versioned trajectories from diverse linear/search teachers and the frozen opponent pool only after the target mechanics profile passes conformance.
- Store all legal afterstate scores/ranks, teacher margin/budget/style, rules/engine hashes, seeds and outcomes in sharded records.
- Train chosen-action BC, full-score/rank distillation and value initialization; then aggregate teacher labels on learner-visited states.
- Compare smoke 100k, pilot 1M and medium 10M decision budgets before approving a larger dataset.

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

1. Obtain a representative pinned-version TETR.IO replay/config export and resolve the target draft's missing timing literals and frame order with fixtures.
2. Implement DAS/ARR/DCD, IRS/IHS, sonic drop, and same-frame conflict normalization above the ordered timing kernel.
3. Connect timing to lock/clear/replay transitions and add an executable first-divergence fixture manifest.
4. Inventory CPU cores, RAM and GPU/VRAM so throughput/latency budgets are numerical.
5. Do not generate heuristic training records until the relevant mechanics profile passes conformance.

## Key risks

- **Upstream opacity/drift:** pin version and store fixtures; never silently follow latest behavior.
- **Replay format/API changes:** use user-owned exports and an internal normalized schema.
- **False equivalence:** publish fixture coverage and confidence, not a blanket unsupported claim.
- **Reward hacking:** terminal objective, potential-only shaping, exact reduced-game checks and opponent-pool evaluation.
- **Self-play cycles:** historical/exploit opponents and held-out style clusters.
- **Compute exhaustion:** native batching, small-model-first successive halving, and hard wall-time budgets.
- **Legal/fair-play concerns:** local-only arena, original assets/naming, license review, no live-service automation.
