# TETR.IO Conformance Strategy

Status: mechanics scope and test strategy settled; rule constants pending evidence
Date: 2026-08-24T15:05:44+09:00

## Meaning of equivalence

No independent implementation can prove identity with undisclosed source/server code. This project therefore uses observational conformance:

> Given the same pinned rules profile, seed, initial state, and ordered frame inputs, the local engine must produce the same observable state/events as the reference for every covered fixture.

This is stricter than visual similarity and honest about unobservable behavior. A rule is not implemented as “TETR.IO-compatible” until its source confidence and differential fixture are recorded.

Conformance covers mechanics that change legal action/reachability, observable transitions, attack/garbage, or round-terminal reward. Accounts, rating, matchmaking, presentation, anti-cheat, and undisclosed server infrastructure are excluded. Round termination remains in scope; service-level match progression does not.

## Evidence hierarchy

1. Official TETR.IO patch notes and data exported by the reference client/replay.
2. Maintainer statements and current TETR.IO Wiki pages that cite them.
3. Repeated black-box fixtures under controlled custom-room settings.
4. Independent open-source implementations and parsers.
5. Reddit/player reports used only to discover edge cases.

Conflicts are resolved upward in this hierarchy. Version/date differences are investigated before calling either source wrong.

## Initial pinned facts

- Season 2 began in Beta 1.2.0 and replaced the old B2B behavior with B2B Charging/Surge, introduced All-Mini behavior, doubled early cancellation under its 14-piece condition, and changed All Clear attack to 5.
- Beta 1.5.0 replaced the multiplayer default with All-Mini+, made immobile T spins that fail the full three-corner test Mini, and restored reworked Clutch Clears. Therefore the current target is All-Mini+, not the launch-era All-Mini profile.
- Beta 1.3.0 added a flat +1 for difficult clears that clear garbage, not influenced by multipliers.
- Default rotation is SRS+, not optional SRS-X.
- Multiplayer piece generation uses a seeded 7-bag; community protocol documentation reports the input order `ZLOSIJT`, which must be confirmed by replay fixtures before becoming `CONFIRMED`.
- Current exact TL timing, cap, messiness, multiplier, top-out, and mechanics-related round parameters are `UNCONFIRMED` until captured from current fixtures.

## Fixture matrix

Each row becomes a minimal replay plus expected event/state hashes:

- All seven piece spawns, four orientations, both wall directions, floor/ceiling and every kick test, including 180 rotations.
- Hold-empty, hold-swap, once-per-piece restriction, IRS/IHS ordering, next queue, bag boundaries, and seed replay.
- DAS charge, ARR 0/nonzero, direction changes, DCD, SDF, gravity, lock delay, reset count, sonic/hard drop, ARE and line-clear delay.
- Single/double/triple/quad, T mini/full spins, All-Mini+ non-T/T immobility cases, last-action/kick-index edge cases, perfect clears.
- Combo values, B2B continuation/break, Surge charge/release/splitting, first-14-piece cancellation boundary, garbage-clear bonus.
- Garbage transit, zero-passthrough behavior, cancellation order, cap, packet boundaries, hole repetition/change, insertion/activation and lethal garbage.
- Block-out/lock-out/partial lock-out/top-out, simultaneous deaths, Clutch Clear/out-of-bounds behavior, and round terminal.
- 40 LINES and BLITZ scoring/gravity/finish; custom option serialization; later, QUICK PLAY as a separate suite.

Boundary tests use `n-1`, `n`, and `n+1` frames/pieces/lines for every threshold.

## Differential harness

The harness consumes sanitized, user-owned replay/config exports; it does not scrape private APIs or control live matches. It normalizes reference events into an internal schema, runs the local engine, and reports the first divergence:

```text
fixture -> normalize -> local replay -> per-frame state hashes
                         |              |
reference checkpoints ---+------ diff -+
```

For state not exposed by the reference, compare the earliest downstream observable and retain `OBSERVED` confidence. Property tests independently enforce conservation and invariants: occupied cells, line-clear compaction, bag permutations, deterministic seed replay, attack/cancellation conservation, and zero-sum results.

## Acceptance gates

- Gate C0: every configured literal has a source/confidence record.
- Gate C1: core geometry, RNG and input timing fixtures pass 100%.
- Gate C2: clear/spin/attack fixtures pass 100%.
- Gate C3: garbage and round-terminal fixtures pass 100% across at least 10,000 seeded randomized differential cases.
- Gate C4: full replay corpus has zero unexplained divergence. Known upstream bugs may be represented only in an explicitly versioned compatibility profile.
- Gate C5: determinism is identical across debug/release builds and supported platforms.

“100%” applies to the declared fixture corpus, not to unknown hidden behavior. Coverage and confidence are published beside the result.
