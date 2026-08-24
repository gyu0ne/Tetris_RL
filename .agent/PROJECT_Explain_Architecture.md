# Project Architecture

Status: score-free clear events and observed TL attack/B2B/combo/Surge transition implemented
Date: 2026-08-24T18:00:13+09:00

## Goal boundary

The system will reproduce a pinned TETR.IO rules profile locally and provide a fast, deterministic 1v1 research arena. Engine equivalence covers mechanics that can affect learning: pieces, randomizer, spawn/hold, SRS+ movement, DAS/ARR/SDF handling, gravity/lock reset, line/spin detection, attacks, B2B/Surge, combo, cancellation, garbage timing/cap/messiness, top-out, round terminal, and replay. Solo play is an unscored mechanics sandbox; mode score systems are not part of the product.

Accounts, matchmaking rating, official servers, cosmetics, audio, anti-cheat, and unknown network implementation are not engine semantics. Match-between-round progression is excluded unless a later learning objective explicitly needs it. Network delay can be simulated as an explicit arena parameter, but it must not be presented as a copy of hidden production behavior.

## Layered design

```text
UI / CLI / experiment runner
             |
        local arena  <----> TBP adapter
             |
    versioned versus rules
             |
 deterministic engine core
             |
 replay/event log + golden fixtures
             |
 native batched simulator bridge
             |
 Python environment -> policy/value training -> evaluator/opponent pool
```

The authoritative engine is a pure state transition `next_state = step(state, ordered_frame_inputs, rules, seed)`. Rendering, clocks, sockets, and learners cannot mutate engine state except through typed commands.

The implemented `crates/engine-core` contains row bitboards, piece geometry, a generic seeded bag, configurable spawn/hold/queue, observed SRS+/180 rotations, geometric reachability, a float-free frame timing kernel, generic ordered-edge/DAS/ARR/DCD/sonic-drop normalization, and held-key IRS/IHS with generic IHS-before-IRS spawn resolution. Its `FrameSession` joins those layers into one continuous transition from ordered input edges through hold, movement/rotation, gravity, lock/line clear and next spawn. The timing state preserves last successful player action, rotation direction and kick index; the lock transition emits a score-free `ClearEvent` containing line count, All-Mini+ spin candidate and post-clear perfect-clear fact, plus lock visibility and typed top-out reasons. `crates/versus` consumes that event and emits ordered integer attack packets for the observed TL base table, multiplier combo, flat B2B, B2B Charging/Surge, perfect clear and garbage-clear special bonus. `crates/rules-tetrio` owns both timing and attack literals with evidence. `crates/replay-conformance` captures canonical frame snapshots and returns the first deterministic mismatch without depending on an undocumented upstream wire format. Replay input is validation-only; no visual player/viewer is part of the engine. Garbage queue, two-player scheduling and learning crates remain subsequent layers. Details include `PROJECT_Explain_Versus_Attack.md`.

## Core representations

- Board: row-oriented bitboard, at least 40 logical rows plus explicit visible/hidden boundaries. Width masks make collision and full-row checks bit operations.
- Time: integer frames/ticks with a documented 60 Hz conversion. Subframe inputs, if evidence requires them, use fixed-point sequence numbers rather than floats.
- Piece: kind, orientation, origin, lock/reset counters, last successful movement/rotation, and kick index needed for spin classification.
- Queue: explicit RNG algorithm, seed, bag state, hold state, and visible next count.
- Garbage: ordered packets with sender, amount, hole seed/column, arrival/activation frame, cancellation state, and profile-specific flags.
- Replay: rules hash, seed, initial state, ordered inputs/events, periodic state hashes, and final result.

## Mode profiles

Rules are data-backed profiles layered over shared mechanics:

1. `modern-core`: 10-column field, tetromino geometry, hold, 7-bag, SRS+/180, frame handling.
2. `tetrio-beta-1_7_8-tl-s2`: Season 2 1v1 All-Mini+, attack, Surge, opener, garbage and round-terminal rules. Values not evidenced by fixtures remain unavailable, not default-guessed.
3. `solo-sandbox`: unscored single-board survival and transition testing over `modern-core`.

40 LINES, BLITZ, ZEN/custom scoring and QUICK PLAY/ROYALE profiles are excluded from the planned product.

## Engine/learning boundary

At every piece decision, the native engine enumerates all reachable locked afterstates, including hold branches, and returns compact features plus an action token. The learner scores this variable action set. Once selected, the movement planner returns an exact frame-input sequence and the engine revalidates it. This prevents the learner from exploiting impossible placements and avoids wasting training on long key-level credit assignment before strategic play is learned.

Frame-level control can be added later as a second-stage imitation/RL problem, benchmarked against the oracle movement planner. It is not required for the first strong strategic bot.

## Technology decision

- Rust workspace for engine, arena, replay, protocol, fuzzing, and Python extension.
- Python and PyTorch for experiment orchestration and model training.
- JSON/TOML for human-reviewed rules and experiment configs; canonical hashes embedded in replays/checkpoints.
- Containerized toolchains and reproducible release builds.

Rust was selected for deterministic low-level control, bit operations, safe parallel simulation, and compatibility with established versus bots such as Cold Clear. Python remains outside the correctness kernel and is used where research iteration matters.

## Non-negotiable interfaces

- `RulesProfile`: immutable/versioned; refuses missing required evidence-backed fields.
- `FrameNormalizer`: converts ordered edges and held state into DAS/ARR/DCD/soft-drop actions plus hold requests; generic spawn processing samples held IHS/IRS in an explicitly provisional IHS→IRS order.
- `FrameSession`: owns continuous `GameState`, optional live `TimingState`, `HandlingState`, and frame index; a step either advances the current piece, commits a lock/clear and spawns the next piece, or reaches terminal state.
- `SpinClassifier`: consumes board-before-lock, final piece, last successful player action and profile rules; returns Mini/Full plus rotation provenance without embedding attack values.
- `AttackResolver`: consumes `ClearEvent`, prior combo/B2B state, garbage-clear context and immutable rules; returns the next state plus ordered Surge, clear and perfect-clear packets without score points or floating point.
- `TopOutRules`: always reports colliding spawn as `BlockOut`; lock-out variants consume pre-clear lock visibility and are enabled only by an explicit profile.
- `ReplayAdapter`: version-pinned converter from a verified user-owned upstream sample into canonical edges, handling and snapshots; it is not implemented by guessing an undocumented `.ttrm` schema.
- `Engine::step`: deterministic and side-effect free except caller-owned state mutation.
- `Engine::legal_afterstates`: same reachability logic used by the input planner.
- `Replay::verify`: detects the first divergent frame and state component.
- `VectorEnv::step_batch`: releases the Python GIL and returns packed observations without per-cell Python objects.
- `Arena::match`: local evaluation orchestration over one or more rounds; accepts explicit seeds, rules hash, latency model, time/node budget, and opponent IDs. It is not a TETR.IO service-system clone.

## Rejected shortcuts

- A single Python grid engine: easy to prototype but too slow and too easy to diverge between training and gameplay.
- Pixel-only CNN with raw key actions: high sample cost and poor credit assignment without proving benefit.
- Copying a fan attack formula as truth: community formulas can be outdated, especially across Season 2.
- Training before conformance fixtures: a fast learner on wrong rules optimizes the wrong game.
