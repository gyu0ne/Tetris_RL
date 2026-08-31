# Versus Attack Discovery r6

## Settled decision

r5 update 230 evidence showed no outgoing-attack improvement, a much smaller offense trace than terminal trace, and near-zero PPO movement. r6 therefore separates the offensive GAE trace from the base policy advantage instead of merely extending r5.

```text
delta = (g0 - g1) / 4
r_attack_pair = +/- 0.06 * sign(delta) * clamp(abs(delta), 0, 1)^1.5
A_policy = std(std(A_total - A_attack) + 0.7 * std(A_attack))
```

The critic still fits the total return. The actor receives an explicit standardized attack channel. Technique labels are not event rewards; only actual post-cancellation outgoing attack enters `r_attack`.

## Exploration and stability

- restore tactical candidate curriculum `0.0003 -> 0.00005` through update 150
- actor LR `3e-4`, solo trunk `0.1x`
- normalized entropy `0.0003 -> 0.0001` through update 200
- target KL `0.003`, stop remaining PPO epochs after a minibatch exceeds `1.5x`
- current/historical/bootstrap fractions `20/65/15`
- initialize from r4 selected, never from the failed r5 trajectory
- hard maximum 200 updates

The finite 200-update boundary replaces an within-run decay requirement for this bounded objective-change experiment. Held-out promotion remains authoritative.

## Opportunity diagnostics

Record attack and spike opportunity rates, capture rates, best-candidate capture, available-outgoing capture ratio, separate advantage standard deviations and their cosine. These distinguish failure to create tactical states from failure to choose an available attack.

## Staged stop and promotion

Probe cumulative updates 50/100/150/200 against fixed r3 update 700 using identical seeds and sides. Stop at update 100 when outgoing attack is below `1.10x` r4. Final promotion requires `1.20x` attack, fixed-anchor score delta at least `-0.03`, direct r4 score at least `0.47`, and danger/holes ratios at most `1.15`, with paired 8/12/15 cadence evaluation.

## Compatibility

- config schema: `versus-selfplay-ppo-v7`
- progress schema: `versus-selfplay-ppo-progress-v6`
- v6 retains its original relative-attack clamp semantics
- v7-only config fields are removed from older serialized payloads
- exact resume includes optimizer, environment, assignments and stable pool state

## Verification contract

- old v6 reward semantics and new convex v7 reward unit tests
- fresh native-environment v7 smoke
- exact v7 resume smoke
- opportunity, separated-advantage, tactical-target and KL log presence
- full Python format/lint/test gate
- staged runner syntax and evaluation CLI argument verification
