# Human-versus-model battle design

Status: implemented

## Boundary

The tool is a local single-round diagnostic match between one keyboard-controlled human and one compatible versus actor checkpoint. It does not reproduce TETR.IO presentation or services and does not change the learned action space.

## Settled design

- Player one is authoritative frame-level `FrameSession` input.
- Player two keeps the reinforcement learner's reachable-placement action space and fixed cadence, defaulting to one placement every 12 engine frames.
- When the model is due, player-one edges and the selected player-two placement enter one transactional battle frame. Attack cancellation, zero-passthrough, garbage insertion, spawn and terminal resolution therefore retain the existing simultaneous semantics.
- Browser code renders snapshots and transmits input edges only. It contains no movement, attack, garbage or terminal rules.
- The Python controller loads one inference checkpoint at process start, scores the Rust-provided 76-integer joint candidate vectors with `actor_logits`, and returns diagnostics. A restart is required to load a newer checkpoint.
- The request loop is back-pressured: it never overlaps frames. Slow inference may reduce wall-clock FPS but cannot skip or reorder authoritative engine frames.

## Rejected alternatives

- Running a second JavaScript engine was rejected because it would duplicate and drift from authoritative mechanics.
- Converting the actor to raw keyboard/finesse output was rejected because the trained policy has no such action head and the user requested direct play against the current model.
- Sequentially applying human and model locks was rejected because it would introduce order-dependent cancellation and terminal behavior.

## Verification gates

- Rust: same-frame dual lock, configured bot cadence, visible state snapshots, existing battle regression suite.
- Python: actor argmax, reset, input validation, checkpoint load.
- Browser: starts paused, key input changes human state, model advances at cadence, no horizontal overflow at 320/375/414/768/1280 widths, no functional console errors.
- Repository: Rust fmt/clippy/test/release build, Python Ruff/full unittest, Compose config.
