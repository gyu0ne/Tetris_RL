# Reinforcement Learning and Reward Design

Status: research-backed candidate plan; final model and weights intentionally unset
Date: 2026-08-24T15:05:44+09:00

## Problem formulation

A TETRA LEAGUE round is modeled as a seeded two-player zero-sum stochastic game. A strategic decision occurs when a piece/hold branch is available. The action set is the engine-enumerated reachable afterstates; the transition includes movement execution, both players' decisions, garbage transit, and the next piece information allowed by the rules.

Observation includes both visible boards, current/hold/next pieces, incoming garbage packets and timing, combo/B2B/Surge state, piece counts, round-terminal context, and legal afterstates. Hidden RNG state is not exposed to the policy. Rating, matchmaking and service UI are outside the learning state.

## Why not default to a CNN

Classic Tetris results show strong performance from compact structural features and afterstate evaluation. Recent bitboard work reports large simulator speedups and better efficiency from afterstate actors; Cold Clear demonstrates the strength of search/evaluation in modern versus play. A raw 2D CNN therefore has no automatic claim to be the best use of limited compute.

The experiment ladder is:

1. Linear evaluator on normalized Dellacherie/Thiery-style and versus features.
2. Small MLP shared across candidate afterstates.
3. Hybrid compact spatial encoder plus scalar context, only if it beats (2) per unit wall time.
4. Shallow search guided by the best scorer; MCTS/beam budget is a controlled inference parameter.

Candidate afterstate features include landing height, eroded cells, row/column transitions, holes and hole depth, cumulative wells, bumpiness, reachable garbage well, attack/cancel amount, board danger, clean/dirty incoming packets, B2B/Surge opportunity, and opponent-relative versions. T-spin pattern features are experimental and require reachability evidence.

The leading trainable candidate is a shared afterstate scorer with an actor softmax over only legal candidates and a centralized training value head. It uses parameter sharing between players, player-swap antisymmetry, and separate inference observations that contain no privileged state. PPO is a candidate because a 2026 bitboard/afterstate study reports sample-efficient results, but it must beat cross-entropy/ES and supervised-search initialization under this versus benchmark before adoption.

## Reward theorem and construction

The unshaped per-transition reward for player 1 is:

```text
r_t = +1 on round win, -1 on loss, 0 on draw or nonterminal step
r_t(player 2) = -r_t(player 1)
```

For an episodic discount `gamma`, an allowed dense term is only:

```text
F(s,a,s') = gamma * Phi(s') - Phi(s)
r'_t = r_t + lambda * F(s,a,s')
```

Ng, Harada and Russell show this potential-difference form preserves optimal policies in an MDP. Because this project is a two-player stochastic game, the direct theoretical basis is Lu, Schwartz and Givigi's Nash-equilibrium invariance result for potential-based reward transformations in stochastic games, supported by Devlin and Kudenko's multi-agent analysis. Under those assumptions, use an antisymmetric potential

```text
Phi(s) = clip(w^T [f(self,s) - f(opponent,s)], -1, 1)
Phi(s_terminal) = 0
Phi(swapped_players(s)) = -Phi(s)
```

so the shaped rewards remain zero-sum. With `gamma = 1` and terminal-normalized potential, the shaping sum telescopes to `-Phi(s_0)`, a policy-independent constant for a fixed initial state. With `gamma < 1`, the discounted sum telescopes analogously. This establishes invariance for the modeled state/action game under the theorem's assumptions; it does not prove that an approximate learner will train faster.

The theorem assumptions, player-swap antisymmetry, and terminal normalization must be checked in code. A reduced finite stochastic game is solved with and without shaping to compare optimal-action/Nash-equilibrium sets before any dense term is accepted.

No direct `+attack`, `-height`, `-holes`, survival, or APM reward is allowed outside `Phi`. Such rewards can favor farming, stalling, or suicidal spikes and change the actual objective.

## Heuristic demonstration bootstrap

Before self-play RL, pretrain the shared afterstate scorer/value head from diverse heuristic and shallow-search teachers. Records contain all legal candidates with teacher scores/ranks and provenance, not only the chosen action. This supports cross-entropy, soft/listwise distillation and search-return value targets from the same data.

Behavior cloning is only initialization. DAgger-style aggregation labels states visited by the learner to reduce sequential covariate shift; terminal-objective RL then removes the teacher ceiling. Teacher identity, frozen rating, style, node budget and top-2 margin are retained so weak or ambiguous demonstrations can be sampled or weighted separately.

The formal pipeline and data schema are in `PROJECT_Explain_Imitation_Bootstrap.md` and `research/IMITATION_BOOTSTRAP_RESEARCH_KO.md`.

## Verifying every reward component

Each feature `f_i` must pass all of the following before receiving nonzero weight:

1. **Definition proof:** deterministic units, range and normalization; boundedness verified by property tests.
2. **Symmetry proof:** player swap negates the potential and terminal potential is zero.
3. **Policy-invariance check:** enumerate a reduced-board finite game, solve unshaped and shaped minimax/optimal policies, and assert the optimal action sets are identical within exact arithmetic.
4. **Marginal experiment:** one-feature-at-a-time and leave-one-out ablations over identical seeds/opponent samples.
5. **Mechanism metrics:** measure win rate plus the predicted mediator (holes, danger integral, attack conversion, cancellation efficiency), gradient signal-to-noise, and episode length.
6. **Statistics:** paired bootstrap confidence intervals and sequentially corrected comparisons; accept only effects whose interval clears the predeclared practical threshold.

Weights are learned or selected on training opponents, then frozen for a held-out opponent/time-control suite. This is the feasible mathematical/empirical interpretation of “verify the effect”; claiming a closed-form proof of neural-training speed for each feature would be false.

## Self-play and evaluation

- Bootstrap from search-generated state/action targets only if it improves held-out play; then fine-tune on the true terminal objective.
- Train against a probabilistic pool of current, historical, exploit and baseline policies to reduce cyclic forgetting and self-play nonstationarity.
- Separate training and evaluation seeds, opponent snapshots, and rules configurations.
- Report paired match win rate, Elo-like rating with uncertainty, exploitability proxies against the pool, APP/APL/PPS, survival, inference latency, nodes per move, samples and joules/wall time where measurable.
- Promotion requires statistically significant improvement against the frozen suite, no major regression against any style cluster, and adherence to a fixed inference budget.

## Compute-efficiency plan

- Native Rust bitboards; batch many independent matches per worker; no rendering during training.
- Packed observations and reusable buffers across the Python boundary; GIL-free batched stepping.
- Enumerate/deduplicate afterstates natively and cache transpositions by `(board, queue, hold, versus_context, rules_hash)`.
- Profile simulation, feature extraction, inference, and optimizer separately. Optimize the measured bottleneck.
- Begin with linear/small models on CPU; use GPU only when batched inference/training throughput wins end-to-end.
- Mixed precision, compilation and asynchronous rollout/learning are optional only after deterministic FP32 baselines.
- Predeclare small/medium/full budgets and use successive halving; do not spend the full budget on an unvalidated architecture.
