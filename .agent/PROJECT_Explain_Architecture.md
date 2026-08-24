# Project Architecture

Status: initial major design completed; deterministic core and timing/profile foundation implemented
Date: 2026-08-24T16:05:13+09:00

## Goal boundary

The system will reproduce a pinned TETR.IO rules profile locally and provide a fast, deterministic 1v1 research arena. Engine equivalence covers mechanics that can affect learning: pieces, randomizer, spawn/hold, SRS+ movement, DAS/ARR/SDF handling, gravity/lock reset, line/spin detection, scoring where relevant, attacks, B2B/Surge, combo, cancellation, garbage timing/cap/messiness, top-out, round terminal, and replay.

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

The implemented `crates/engine-core` contains row bitboards, piece geometry, a generic seeded bag, configurable spawn/hold/queue, observed SRS+/180 rotations, geometric reachability, and a float-free frame timing kernel. `crates/rules-tetrio` is a one-way adapter layer whose evidence-bearing draft can create core timing rules only when every required literal is present. Versus, replay and learning crates remain subsequent layers. Details are in `PROJECT_Explain_Engine_Core_Phase1.md` and `PROJECT_Explain_Timing_and_Rules_Profile.md`.

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
3. `40-lines`, `blitz`, `zen-custom`: mode-specific gravity, scoring, goal, and option surfaces.
4. `quick-play-royale`: deferred independent profile due to height multipliers, targeting, mods, and other non-1v1 behavior.

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
- `FrameNormalizer`: converts held/raw inputs and profile ordering into the ordered discrete actions consumed by the timing kernel; exact TETR.IO ordering remains fixture-gated.
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
