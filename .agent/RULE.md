# Project Rules

Last updated: 2026-08-24T20:29:16+09:00

## 1. Authority and scope

- This file is the repository-wide working agreement. A direct user instruction overrides it; the conflict and resolution must be logged in `CONTINUITY.md`.
- The product is a local, independently implemented falling-block engine and a bot arena. It must not automate, inject into, or play on the live TETR.IO service.
- “TETR.IO-equivalent” means functional/observational conformance to a pinned public TETR.IO version and mode profile. TETR.IO 운영자의 승인·서명·공식 인증은 완료 조건이 아니다. Unpublished server behavior must be labeled `UNCONFIRMED`, never guessed.
- The initial conformance target is `TETR.IO BETA 1.7.8 / TETRA LEAGUE Season 2`, pending capture of authoritative replay/config fixtures. A later upstream patch does not silently change the target.
- Equivalence covers only mechanics that can change legal actions/reachability, observable state transitions, attack/garbage behavior, or round-terminal reward. Accounts, rating, matchmaking, UI/audio/cosmetics, anti-cheat, and undisclosed service infrastructure are excluded.
- Round/top-out/simultaneous-death/Clutch-Clear semantics remain in scope because they determine terminal reward. Match-level progression stays out of scope unless an explicit learning objective needs it.
- Solo play is an unscored engine sandbox for survival, transition inspection, and bot smoke tests. 40 LINES, BLITZ, ZEN/custom scoring and goal profiles are excluded unless the user explicitly restores them.
- The current multiplayer spin target is `All-Mini+`, introduced in BETA 1.5.0. Historical Season 2 `All-Mini` behavior must not be used as the current default.

## 2. Required repository structure

Production code must be separated by responsibility:

```text
crates/
  engine-core/       # deterministic board, pieces, movement, timing
  rules-tetrio/      # versioned TETR.IO mode/rules profiles
  replay-conformance/# canonical frame snapshots and differential reports
  versus/            # attacks, garbage queues and round-terminal mechanics
  replay/            # deterministic event log and validation fixtures
  arena/             # local bot-vs-bot orchestration
  bot-protocol/      # TBP-compatible adapter and local protocol
  py-bridge/         # narrow Python binding for vectorized simulation
python/tetris_rl/
  envs/              # Gym-style batched environments
  features/          # proven/experimental state features
  models/            # policy/value models only
  training/          # self-play, opponent pool, checkpoints
  evaluation/        # rating, ablations, statistical reports
configs/             # immutable, versioned rules and experiment configs
tests/
  unit/ property/ differential/ replay/ golden/ fuzz/
benchmarks/          # reproducible engine/training throughput benchmarks
research/            # experiment manifests and generated reports, not code
.agent/              # rules, continuity, plans and design explanations
```

- Do not mix rendering/UI, game rules, learning code, or experiment outputs.
- Generated datasets, checkpoints, caches, and benchmark artifacts must not be committed unless explicitly approved.
- Dependencies and toolchains run in containers by default. Do not install host system packages.

## 3. Change control and continuity

- Read `.agent/CONTINUITY.md` at the start of every work turn.
- Log meaningful creates, edits, moves, deletions, dependency changes, rules changes, design decisions, failed verification, and project-wide effects in `.agent/CONTINUITY.md` with ISO timestamp, source tag, reason, and evidence.
- Keep continuity factual and compact. Use only `[USER]`, `[CODE]`, `[TOOL]`, or `[ASSUMPTION]`; mark unknowns `UNCONFIRMED`.
- After a major design is approved or completed, create or update `.agent/PROJECT_Explain_<detail>.md`. These documents explain settled decisions, alternatives, constraints, interfaces, and verification gates.
- Use small patch-style edits. Preserve unrelated user changes. Deletion or replacement requires an exact target check and a continuity entry.

## 4. TETR.IO conformance rules

- Source priority: official TETR.IO patch notes/exported configurations/replays; official or maintainer statements; current TETR.IO wiki; reproducible black-box fixtures; open-source implementations; community reports.
- GitHub and Reddit claims are hypotheses until corroborated by a higher-priority source or a reproducible differential test.
- Every rules profile must contain its source URL, access date, upstream version, confidence (`CONFIRMED`, `OBSERVED`, `UNCONFIRMED`), and fixture IDs.
- A named TETR.IO profile must refuse executable activation while any transition-critical required field is missing; historical or provisional literals may run only under an explicitly non-conformant test profile.
- A complete `OBSERVED` client-derived profile may run local mechanics tests, but it remains distinct from functionally verified `CONFIRMED` conformance. 여기서 `CONFIRMED`는 운영자 인증이 아니라 version-pinned reference trace와의 exact differential 결과다. Promotion requires reference trace/config fixtures and zero unexplained differential divergence.
- When `room_handling` is disabled, room ARR/DAS/SDF fields are inactive metadata; effective handling must come from the player or replay profile.
- Randomness must be explicit and seedable. A replay must reproduce piece sequence, per-frame inputs, locks, clears, attacks, garbage columns, top-out, and round result byte-for-byte.
- Core geometry, timers and random state use integer/fixed-point units. A pinned upstream JavaScript floating expression or repeated update may use audited IEEE-754 `f64` only when its rounding is the compatibility behavior itself, with finite/range guards and boundary fixtures.
- Spin classification must use the last successful player action plus rotation direction/kick index. Automatic gravity does not erase rotation provenance; a hard drop erases it only when it actually translates the piece.
- Perfect clear is evaluated from the board after line compaction. Block-out, lock-out and partial lock-out remain distinct typed outcomes; unconfirmed target variants must stay disabled rather than being guessed.
- `engine-core::ClearEvent` contains only transition facts. Attack/B2B/combo/Surge and ordered attack packets belong to `versus`; solo score points must not leak into either layer.
- Authoritative attack transitions use checked integers/fixed-capacity packets except for pinned JavaScript floating compatibility paths covered by boundary fixtures; no platform-dependent fast-math optimization is allowed.
- Garbage is represented as ordered packets. Cancellation must preserve attack packet order, consume attack budget before opener-only budget, and update round sent totals between packets.
- Board occupancy and garbage provenance are separate aligned bit layers. Garbage clear bonuses must come from compacted lock provenance, never from stack-shape inference.
- Transit eligibility, cancellation eligibility and insertion eligibility are distinct states. Under the observed TL zero-passthrough profile an in-transit packet may cancel, but it may not rise before its ready frame.
- Raw held-key interpretation and same-frame conflict ordering are versioned normalization responsibilities; the core timing kernel consumes an already ordered discrete action sequence.
- Do not guess undocumented `.ttr`/`.ttrm` fields. Build an upstream adapter only from a version-identified, user-owned sample; keep raw replay exports ignored and commit only anonymized normalized fixtures with provenance and hashes.
- Replay exports are validation oracles, not product requirements. Do not build a visual replay player/viewer unless the user explicitly requests it; a headless test adapter may reapply input events only to validate engine transitions.
- Functional conformance uses `Incomplete`/`Divergent`/`Conformant` only. `Conformant` requires all `REQUIRED_MECHANIC_CLAIMS` to be covered by passing reference cases, at least 10,000 passing randomized battle cases under the default policy, and zero mismatch across the supplied corpus; it must never be described as operator certification.
- Conformance changes require golden fixtures and differential tests before merge. “Looks/feels the same” is not evidence.
- Required profiles are staged: common modern core, unscored solo sandbox, and TETRA LEAGUE Season 2 1v1. 40 LINES, BLITZ, ZEN/custom scoring, and QUICK PLAY/ROYALE are out of scope.
- A candidate feature is in conformance scope when it changes legal action/reachability, observation/transition/RNG, attack/garbage timing, or round terminal. Convenience tooling and service presentation are not conformance features.

## 5. Learning and reward rules

- Do not select a CNN, algorithm, reward term, or hyperparameter solely by convention.
- Maintain non-learning baselines: random legal, linear Dellacherie/Thiery-style evaluator, beam/MCTS bot, and an archived external-bot protocol baseline where licensing permits.
- The primary action abstraction is selection among reachable locked afterstates. A separate deterministic movement planner converts the selection into legal frame inputs and can reject unreachable actions.
- The default learning candidate is a small shared afterstate scorer plus actor-critic value head, tested against linear and small spatial encoders. The final model is selected only by held-out match strength, sample efficiency, inference latency, and ablation evidence.
- The unshaped objective is zero-sum terminal round outcome. Dense shaping is allowed only when it is potential-based, `F(s,a,s') = gamma*Phi(s') - Phi(s)`, antisymmetric across players, bounded, terminal-normalized, and accompanied by an MDP/stochastic-game policy or Nash-equilibrium invariance check.
- Every proposed potential feature must define units, normalization, bound, expected causal effect, failure mode, and an ablation with confidence intervals. Correlation alone is not causation.
- Evaluate against frozen historical opponents and baselines, never only the current self-play policy. Seeds, code revision, configuration hash, hardware, wall time, and confidence interval must be reported.
- Before RL, bootstrap candidates from versioned heuristic/search demonstrations. Store every legal candidate score/rank plus rules, engine, seed, opponent and teacher hashes; split datasets by match/seed, not individual rows.
- One-shot behavior cloning is not sufficient evidence. Compare chosen-only cloning, full-score distillation, learner-state dataset aggregation, and terminal-objective RL fine-tuning in closed-loop matches.
- Generated datasets/checkpoints stay outside version control; commit schemas, manifests, configs and reports only.
- Training performance work follows measurement: profile first, then optimize. Prefer bitboards, batched native simulation, zero-copy observations, vectorized environments, mixed precision where numerically safe, and bounded opponent pools.

## 6. Verification and completion

- For source changes, attempt format, lint, unit/property tests, type checks, release build, conformance replay suite, and relevant benchmarks. Resolve failures or explicitly record them as out of scope.
- Rule correctness gates precede RL training. No model may train on an engine that has not passed the deterministic and conformance gates for its profile.
- Every experiment must be reproducible from a committed config and container image digest.
- A phase is complete only when its acceptance criteria and evidence are recorded in `CONTINUITY.md` and the corresponding `PROJECT_Explain_*.md`.
