# Versus Offense Generalization r8

## Decision evidence

The r7 update-75 attack checkpoint was re-evaluated at the actual deployment cadence of 12 frames per placement using 64 independent held-out seeds per matchup and a 4,000-decision horizon. Every game completed. Side swapping produced identical deterministic paired outcomes, so the effective sample size is 64 rather than incorrectly treating the duplicated sides as 128 independent games.

Against r4 directly, r7 scored `0.5469`. Across the two fixed r3 anchors, outgoing attack increased from `0.14446` to `0.15893` per piece (`1.1002x`) while mean score changed from `0.3984` to `0.3828`. The key failure is non-transitive: score against r3 update 700 fell from `0.4531` to `0.2813`, even though score against update 1050 rose from `0.3438` to `0.4844`. Production promotion is therefore rejected.

## r8 objective

r8 starts from `r7/aggressive-model.pt`, not r4. It preserves the r7 convex net-outgoing reward and the attack channels:

```text
std(1.0*std(A_base) + 2.0*std(A_setup) + 3.0*std(A_attack))
```

The base coefficient increases from `0.25` to `1.0`. This does not make survival the dominant objective: setup plus attack still carry five standardized units versus one base unit. It restores enough terminal, defense and critic-correction credit to address the r3-700 collapse.

## Opponent and optimization changes

- current/historical/bootstrap: `10/85/5%`
- historical balanced/hard/uniform: `20/70/10%`
- PFSP exponent: `2.0`, emphasizing opponents with low learner score
- score half-life: `50` updates, so repaired weaknesses are re-measured promptly
- actor learning rate: `2e-4`
- solo trunk multiplier: `0.5`
- target KL: `0.003`
- entropy: `0.0003 -> 0.0001`
- maximum retained stages: `25, 50, 75, 100, 125, 150`

## Promotion boundary

r8 is target-cadence-specific and does not claim 8/15-frame robustness. Final 12-frame promotion requires:

- fixed-anchor mean and worst-opponent score delta at least `-0.02`
- direct r4 score at least `0.50`
- outgoing attack at least `1.05x` r4, approximately retaining 95% of the measured r7 attack level
- danger ratio at most `1.50`
- holes ratio at most `1.25`

No passing candidate leaves the r4 model as `selected-model.pt`. Stage probes are diagnostics; only the final fixed-anchor selector may promote.

## Execution

```powershell
./scripts/run-versus-offense-generalization-r8.ps1 -ResourceProfile max
```

The runner requires a clean tree, initializes a new r8 directory from r7 update 75, resumes exact progress when present, evaluates every 25 updates against r3 update 700 and r4, then runs the final 12-frame selector.
