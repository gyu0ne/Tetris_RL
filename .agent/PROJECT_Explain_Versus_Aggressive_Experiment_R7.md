# Versus Aggressive Experiment r7

## Evidence and objective change

r6 stopped correctly at update 100 with held-out attack ratio `1.0043`. Its attack capture remained roughly 80% and spike capture roughly 100%, while attack opportunity was roughly 10% and spike opportunity roughly 0.5%. The failure was state construction, not immediate attack selection.

r7 is an explicit attack-first research objective. It partitions total advantage into base, attack-setup and realized-attack traces, then applies:

```text
std(0.25*std(A_base) + 2.0*std(A_setup) + 3.0*std(A_attack))
```

`A_setup` consists of existing combo, B2B, attack-readiness and Full-T-spin-readiness potential components. It is not added to total reward twice. The critic retains total return.

## Aggressive mechanics

- convex net outgoing event reward coefficient `0.10`, power `2.0`
- solo trunk LR multiplier `1.0`, actor LR `4e-4`
- tactical fraction `0.50`, tactical curriculum `0.003 -> 0.0005`, temperature `0.25`
- normalized entropy `0.0005 -> 0.0002`
- target KL `0.006`
- current/historical/bootstrap `10/75/15`
- maximum 100 updates from r4 selected

## Artifact boundary

Stages 25/50/75/100 are evaluated against the same fixed r3 opponent and seeds. `aggressive-model.pt` always retains the highest-attack stage and is explicitly not a production promotion. `selected-model.pt` remains gated by attack, score, direct-r4, danger and holes checks. This separation allows deliberate offensive behavior to be inspected without silently replacing the stable model.

## Compatibility

- config schema `versus-selfplay-ppo-v8`
- progress schema `versus-selfplay-ppo-progress-v7`
- v1-v7 serialized payloads exclude r7-only base/setup fields
- v7 policy advantage and v6 reward semantics remain unchanged

## Verification contract

- setup component selection and terminal-zero test
- aggressive three-channel policy direction test
- fresh and exact-resume native v8 smoke
- logs for setup scale, alignment, opportunity and KL
- full Python/Rust/container gates
- staged PowerShell runner syntax and checkpoint identity checks
