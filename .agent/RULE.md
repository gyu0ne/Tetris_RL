# Project Rules

Last updated: 2026-08-31T09:51:16+09:00

## 1. Authority and scope

- This file is the repository-wide working agreement. A direct user instruction overrides it; the conflict and resolution must be logged in `CONTINUITY.md`.
- The product is a local, independently implemented falling-block engine and a bot arena. It must not automate, inject into, or play on the live TETR.IO service.
- “TETR.IO-equivalent” means functional/observational conformance to a pinned public TETR.IO version and mode profile. TETR.IO 운영자의 승인·서명·공식 인증은 완료 조건이 아니다. Unpublished server behavior must be labeled `UNCONFIRMED`, never guessed.
- The initial conformance target is `TETR.IO BETA 1.7.8 / TETRA LEAGUE Season 2`, pending capture of authoritative replay/config fixtures. A later upstream patch does not silently change the target.
- Equivalence covers only mechanics that can change legal actions/reachability, observable state transitions, attack/garbage behavior, or round-terminal reward. Accounts, rating, matchmaking, UI/audio/cosmetics, anti-cheat, and undisclosed service infrastructure are excluded.
- Pixel, animation, layout, skin and audiovisual parity are never conformance requirements. A diagnostic UI may use simplified colors and geometry as long as it displays authoritative engine state without reimplementing mechanics.
- Round/top-out/simultaneous-death/Clutch-Clear semantics remain in scope because they determine terminal reward. Match-level progression stays out of scope unless an explicit learning objective needs it.
- Solo play is an unscored engine sandbox for survival, transition inspection, and bot smoke tests. 40 LINES, BLITZ, ZEN/custom scoring and goal profiles are excluded unless the user explicitly restores them.
- The current multiplayer spin target is `All-Mini+`, introduced in BETA 1.5.0. Historical Season 2 `All-Mini` behavior must not be used as the current default.

## 2. Required repository structure

Production code must be separated by responsibility:

```text
crates/
  engine-core/       # deterministic board, pieces, movement, timing
  manual-playground/ # local diagnostic UI backed by engine-core
  rules-tetrio/      # versioned TETR.IO mode/rules profiles
  replay-conformance/# canonical frame snapshots and differential reports
  versus/            # attacks, garbage queues and round-terminal mechanics
  replay/            # deterministic event log and validation fixtures
  arena/             # local bot-vs-bot and human-vs-bot orchestration
  bot-protocol/      # TBP-compatible adapter and local protocol
  py-bridge/         # narrow Python binding for vectorized simulation
python/tetris_rl/
  envs/              # Gym-style batched environments
  features/          # proven/experimental state features
  models/            # policy/value models only
  training/          # self-play, opponent pool, checkpoints
  evaluation/        # rating, ablations, statistical reports
  human_battle/      # local HTTP controller and diagnostic battle UI
configs/             # immutable, versioned rules and experiment configs
tests/
  unit/ property/ differential/ replay/ golden/ fuzz/
benchmarks/          # reproducible engine/training throughput benchmarks
research/            # experiment manifests and generated reports, not code
Explanation/         # Korean standalone explanations for implemented components/workflows
.agent/              # rules, continuity, plans and design explanations
```

- Do not mix rendering/UI, game rules, learning code, or experiment outputs.
- Manual tools call the same Rust mechanics APIs used by tests/arena; a separately implemented browser/JavaScript rules engine is prohibited.
- Human-versus-model play keeps human controls frame-level and model decisions placement-level. Attack, cancellation, garbage, top-out and simultaneous resolution must remain a single transactional `BattleSession` transition; the browser may never approximate them.
- Generated datasets, checkpoints, caches, and benchmark artifacts must not be committed unless explicitly approved.
- Dependencies and toolchains run in containers by default. Do not install host system packages.

## 3. Change control and continuity

- Read `.agent/CONTINUITY.md` at the start of every work turn.
- Log meaningful creates, edits, moves, deletions, dependency changes, rules changes, design decisions, failed verification, and project-wide effects in `.agent/CONTINUITY.md` with ISO timestamp, source tag, reason, and evidence.
- Keep continuity factual and compact. Use only `[USER]`, `[CODE]`, `[TOOL]`, or `[ASSUMPTION]`; mark unknowns `UNCONFIRMED`.
- After a major design is approved or completed, create or update `.agent/PROJECT_Explain_<detail>.md`. These documents explain settled decisions, alternatives, constraints, interfaces, and verification gates.
- Every newly implemented engine, model, training, evaluation, or operational workflow must also have a separate Korean standalone explanation under `Explanation/`. Use one topic per file, update the same file when that topic changes, and keep `Explanation/README.md` as the index. `.agent/PROJECT_Explain_*.md` remains the internal design-decision record; `Explanation/` is the user-readable technical manual.
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
- The formal `Conformant` label is a confidence report, not a blocker for manual playtesting, arena construction, heuristic teacher development or clearly labeled exploratory imitation runs. Core automated tests and manual smoke checks remain mandatory before those activities; final equivalence claims still require the formal gate.
- Conformance changes require golden fixtures and differential tests before merge. “Looks/feels the same” is not evidence.
- Required profiles are staged: common modern core, unscored solo sandbox, and TETRA LEAGUE Season 2 1v1. 40 LINES, BLITZ, ZEN/custom scoring, and QUICK PLAY/ROYALE are out of scope.
- A candidate feature is in conformance scope when it changes legal action/reachability, observation/transition/RNG, attack/garbage timing, or round terminal. Convenience tooling and service presentation are not conformance features.

## 5. Learning and reward rules

- Do not select a CNN, algorithm, reward term, or hyperparameter solely by convention.
- Maintain non-learning baselines: random legal, linear Dellacherie/Thiery-style evaluator, beam/MCTS bot, and an archived external-bot protocol baseline where licensing permits.
- The primary action abstraction is selection among reachable locked afterstates. A separate deterministic movement planner converts the selection into legal frame inputs and can reject unreachable actions.
- The learning policy selects only `hold + piece + orientation + x/y`; movement paths and finesse are diagnostics/execution concerns, never policy inputs. Placement-level arena time advances by an explicit shared cadence rather than pretending that a direct afterstate choice was a raw TETR.IO input sequence.
- The first versus cadence default is 12 frames per placement (5 PPS at 60 Hz), with 8/12/15-frame sensitivity tests required before treating a result as cadence-robust. This is a local arena budget, not a TETR.IO mechanics claim.
- The default learning candidate is a small shared afterstate scorer plus actor-critic value head, tested against linear and small spatial encoders. The final model is selected only by held-out match strength, sample efficiency, inference latency, and ablation evidence.
- The unshaped objective is zero-sum terminal round outcome. Dense shaping is allowed only when it is potential-based, `F(s,a,s') = gamma*Phi(s') - Phi(s)`, antisymmetric across players, bounded, terminal-normalized, and accompanied by an MDP/stochastic-game policy or Nash-equilibrium invariance check.
- A non-potential offense curriculum is allowed only by an explicit objective-change decision. It must use the two players' relative post-cancellation outgoing attack, remain zero-sum and bounded, decay on a committed schedule, avoid technique-label/gross-clear rewards, and fail back to the unmodified baseline unless held-out score, direct-baseline, attack and stability promotion gates all pass.
- Every proposed potential feature must define units, normalization, bound, expected causal effect, failure mode, and an ablation with confidence intervals. Correlation alone is not causation.
- Evaluate against frozen historical opponents and baselines, never only the current self-play policy. Seeds, code revision, configuration hash, hardware, wall time, and confidence interval must be reported.
- Historical opponent membership must be persisted and change gradually; recomputing a bounded pool in a way that replaces many opponents at one snapshot boundary is prohibited. Current-policy matchup estimates must expire or be re-evaluated rather than combining all learner generations indefinitely.
- Final versus promotion must compare a bounded shortlist against fixed anchors and peer candidates with paired sides and cadence sensitivity. The latest snapshot or one training metric may not be promoted automatically.
- A self-play match keeps its opponent kind/checkpoint and learner side fixed until terminal. Update boundaries and resume must persist this assignment; changing an opponent during an active match invalidates terminal credit.
- Variable-length action entropy must be reported both raw and normalized by `log(legal candidate count)`. Entropy coefficients and schedules are committed experiment semantics, not resource-profile controls.
- Before RL, bootstrap candidates from versioned heuristic/search demonstrations. Store every legal candidate score/rank plus rules, engine, seed, opponent and teacher hashes; split datasets by match/seed, not individual rows.
- One-shot behavior cloning is not sufficient evidence. Compare chosen-only cloning, full-score distillation, learner-state dataset aggregation, and terminal-objective RL fine-tuning in closed-loop matches.
- Generated datasets/checkpoints stay outside version control; commit schemas, manifests, configs and reports only.
- Demonstration shards are ephemeral by default. A retained checkpoint must embed the dataset/config hash, rules and engine provenance, feature normalization, teacher identity and training settings so deterministic shards can be deleted and regenerated.
- Every generated game in one training dataset must have a distinct recorded seed. A fixed `base_seed` identifies a reproducible schedule; it must never mean reusing one game seed across matches.
- The final solo bootstrap uses independent initialization runs, validation-selected best epochs and actual engine closed-loop selection. Any top-out in its final zero-top-out gate routes to learner-state aggregation; it must not be hidden by lowering the gate.
- Long-running trainers must save an atomic progress checkpoint after every completed epoch and support exact continuation of model, optimizer, shuffle epoch, early-stopping state and history. A completed candidate checkpoint supersedes and removes its progress checkpoint.
- Resource profiles may change independent-run concurrency, native thread counts and evaluation parallelism only. They must not silently change batch size, learning rate, epoch budget, seed schedule or any other learning-semantic setting. Resuming with a different resource profile is allowed; resuming with incompatible data or semantic settings must fail closed.
- Training performance work follows measurement: profile first, then optimize. Prefer bitboards, batched native simulation, zero-copy observations, vectorized environments, mixed precision where numerically safe, and bounded opponent pools.

## 6. Verification and completion

- For source changes, attempt format, lint, unit/property tests, type checks, release build, conformance replay suite, and relevant benchmarks. Resolve failures or explicitly record them as out of scope.
- Deterministic core tests and manual mechanics checks precede dataset generation. Exploratory heuristic/imitation work may use the `OBSERVED_NOT_FUNCTIONALLY_VERIFIED` profile when prominently labeled; final benchmark claims and release training require the declared conformance gate.
- Every experiment must be reproducible from a committed config and container image digest.
- A phase is complete only when its acceptance criteria and evidence are recorded in `CONTINUITY.md` and the corresponding `PROJECT_Explain_*.md`.
