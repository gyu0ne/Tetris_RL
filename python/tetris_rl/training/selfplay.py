import argparse
import hashlib
import json
import math
import os
import random
import time
from copy import deepcopy
from dataclasses import asdict, dataclass
from pathlib import Path

import torch
from torch import Tensor
from torch.distributions import Categorical

from tetris_rl.envs import VersusObservation, VersusVectorEnv
from tetris_rl.features import MECHANICS_STATUS
from tetris_rl.models import VersusActorCritic, load_scorer, load_versus_actor
from tetris_rl.training.reward import PotentialConfig, transition_reward

CHECKPOINT_SCHEMA = "versus-selfplay-ppo-progress-v2"


@dataclass(frozen=True)
class SelfPlayConfig:
    schema_version: str
    frames_per_placement: int
    parallel_matches: int
    rollout_steps: int
    ppo_epochs: int
    minibatch_decisions: int
    learning_rate: float
    gamma: float
    gae_lambda: float
    clip_ratio: float
    entropy_coefficient: float
    value_coefficient: float
    max_grad_norm: float
    shaping_scale: float
    base_seed: int
    seed_stride: int
    snapshot_interval_updates: int
    self_play_fraction: float
    historical_fraction: float
    opponent_pool_limit: int
    entropy_coefficient_final: float | None = None
    entropy_decay_updates: int = 1
    normalize_entropy: bool = False

    @classmethod
    def load(cls, path: Path) -> "SelfPlayConfig":
        payload = json.loads(path.read_text(encoding="utf-8"))
        config = cls(**payload)
        config.validate()
        return config

    def validate(self) -> None:
        if self.schema_version not in {"versus-selfplay-ppo-v1", "versus-selfplay-ppo-v2"}:
            raise ValueError("unsupported self-play config schema")
        positive = (
            self.frames_per_placement,
            self.parallel_matches,
            self.rollout_steps,
            self.ppo_epochs,
            self.minibatch_decisions,
            self.snapshot_interval_updates,
            self.seed_stride,
            self.opponent_pool_limit,
        )
        if any(value <= 0 for value in positive):
            raise ValueError("integer training settings must be positive")
        if not 0.0 < self.gamma <= 1.0 or not 0.0 <= self.gae_lambda <= 1.0:
            raise ValueError("invalid discount settings")
        if not 0.0 <= self.self_play_fraction <= 1.0:
            raise ValueError("self_play_fraction must be in [0, 1]")
        if not 0.0 <= self.historical_fraction <= 1.0:
            raise ValueError("historical_fraction must be in [0, 1]")
        if self.self_play_fraction + self.historical_fraction > 1.0:
            raise ValueError("opponent fractions exceed one")
        if self.entropy_coefficient < 0.0:
            raise ValueError("entropy coefficient must be nonnegative")
        if self.schema_version == "versus-selfplay-ppo-v2":
            if self.entropy_coefficient_final is None or self.entropy_coefficient_final < 0.0:
                raise ValueError("v2 requires a nonnegative final entropy coefficient")
            if self.entropy_decay_updates <= 0 or not self.normalize_entropy:
                raise ValueError("v2 requires normalized entropy and positive decay updates")


@dataclass(frozen=True)
class OpponentPoolEntry:
    checkpoint: str
    model: VersusActorCritic


@dataclass(frozen=True)
class MatchAssignment:
    kind: str
    learner_side: int | None
    opponent_checkpoint: str | None = None

    def validate(self) -> None:
        if self.kind == "self_play":
            if self.learner_side is not None or self.opponent_checkpoint is not None:
                raise ValueError("self-play assignment cannot name one learner side or opponent")
            return
        if self.kind not in {"bootstrap", "historical"} or self.learner_side not in {0, 1}:
            raise ValueError("invalid fixed-opponent assignment")
        if (self.kind == "historical") != (self.opponent_checkpoint is not None):
            raise ValueError("historical assignment checkpoint mismatch")


@dataclass
class DecisionTransition:
    candidates: Tensor
    state: Tensor
    action: int
    old_log_probability: float
    old_value: float
    reward: float
    next_value: float
    terminal: bool
    is_learner: bool
    advantage: float = 0.0
    return_target: float = 0.0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Placement-level PPO self-play bootstrap")
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--bootstrap", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--hours", type=float, default=24.0)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--max-updates", type=int, default=0)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--initialize-from", type=Path)
    parser.add_argument("--allow-observed", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.hours <= 0.0 or args.threads <= 0 or args.max_updates < 0:
        raise ValueError("hours/threads must be positive and max-updates nonnegative")
    if args.resume and args.initialize_from is not None:
        raise ValueError("--resume and --initialize-from are mutually exclusive")
    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    config = SelfPlayConfig.load(args.config)
    torch.manual_seed(config.base_seed)
    random.seed(config.base_seed)
    solo = load_scorer(args.bootstrap, allow_observed=args.allow_observed)
    model = VersusActorCritic(solo)
    bootstrap_opponent = VersusActorCritic(solo)
    bootstrap_opponent.eval()
    for parameter in bootstrap_opponent.parameters():
        parameter.requires_grad_(False)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    progress_path = args.output_dir / "latest.pt"
    config_hash = _sha256_file(args.config)
    bootstrap_hash = _sha256_file(args.bootstrap)
    initialization_hash: str | None = None
    if not args.resume and args.initialize_from is not None:
        initialized = load_versus_actor(args.initialize_from, allow_observed=args.allow_observed)
        model.load_state_dict(initialized.model.state_dict(), strict=True)
        initialization_hash = _sha256_file(args.initialize_from)
    optimizer = torch.optim.Adam(model.parameters(), lr=config.learning_rate)
    update = 0
    environment_steps = 0
    seed_index = 0
    history: list[dict[str, float | int | str | None]] = []
    match_assignments: list[MatchAssignment]

    if args.resume and progress_path.exists():
        payload = torch.load(progress_path, map_location="cpu", weights_only=True)
        _validate_resume(payload, config, config_hash, bootstrap_hash)
        model.load_state_dict(payload["model_state"], strict=True)
        optimizer.load_state_dict(payload["optimizer_state"])
        update = int(payload["update"])
        environment_steps = int(payload["environment_steps"])
        seed_index = int(payload["seed_index"])
        history = list(payload.get("history", []))
        initialization_hash = payload.get("initialization_sha256")  # type: ignore[assignment]
        match_assignments = [
            MatchAssignment(**assignment) for assignment in payload["match_assignments"]
        ]
        for assignment in match_assignments:
            assignment.validate()
        torch.set_rng_state(payload["torch_rng_state"])
        random.setstate(_as_random_state(payload["python_rng_state"]))
        env = VersusVectorEnv.restore(payload["environment_state"])
    else:
        if args.resume:
            raise FileNotFoundError(f"resume checkpoint not found: {progress_path}")
        if progress_path.exists():
            raise FileExistsError("output already has progress; use --resume or a new directory")
        initial_seeds = [
            _scheduled_seed(config, offset) for offset in range(config.parallel_matches)
        ]
        seed_index = config.parallel_matches
        env = VersusVectorEnv(initial_seeds, config.frames_per_placement)
        historical = _load_opponent_pool(
            args.output_dir, config.opponent_pool_limit, args.allow_observed
        )
        match_assignments = [
            _new_match_assignment(config, index, seed, historical)
            for index, seed in enumerate(initial_seeds)
        ]
        _save_inference(
            args.output_dir / "reference-model.pt",
            model,
            config,
            config_hash,
            bootstrap_hash,
            initialization_hash,
            update,
            environment_steps,
        )

    if len(match_assignments) != env.match_count:
        raise ValueError("match assignment count differs from restored environment")

    observation = env.observe()
    reward_config = PotentialConfig(gamma=config.gamma, shaping_scale=config.shaping_scale)
    deadline = time.monotonic() + args.hours * 3600.0

    try:
        while time.monotonic() < deadline and (args.max_updates == 0 or update < args.max_updates):
            started = time.monotonic()
            model_before = deepcopy(model.state_dict())
            optimizer_before = deepcopy(optimizer.state_dict())
            torch_rng_before = torch.get_rng_state()
            python_rng_before = random.getstate()
            seed_index_before = seed_index
            environment_before = env.state_dict()
            assignments_before = list(match_assignments)
            try:
                historical = _load_opponent_pool(
                    args.output_dir, config.opponent_pool_limit, args.allow_observed
                )
                transitions, observation, seed_index, simulated_decisions, rollout_metrics = (
                    _collect_rollout(
                        env,
                        observation,
                        model,
                        match_assignments,
                        bootstrap_opponent,
                        historical,
                        args.allow_observed,
                        config,
                        reward_config,
                        seed_index,
                    )
                )
                entropy_coefficient = _entropy_coefficient(config, update)
                metrics = _ppo_update(model, optimizer, transitions, config, entropy_coefficient)
                metrics.update(rollout_metrics)
            except BaseException:
                model.load_state_dict(model_before, strict=True)
                optimizer.load_state_dict(optimizer_before)
                torch.set_rng_state(torch_rng_before)
                random.setstate(python_rng_before)
                seed_index = seed_index_before
                match_assignments = assignments_before
                env = VersusVectorEnv.restore(environment_before)
                observation = env.observe()
                raise
            update += 1
            environment_steps += simulated_decisions
            metrics.update(
                {
                    "event": "training_update",
                    "update": update,
                    "environment_steps": environment_steps,
                    "entropy_coefficient": entropy_coefficient,
                    "seconds": time.monotonic() - started,
                }
            )
            _add_rolling_scores(metrics, history)
            history.append(metrics)
            history = history[-100:]
            _save_progress(
                progress_path,
                model,
                optimizer,
                config,
                config_hash,
                bootstrap_hash,
                update,
                environment_steps,
                seed_index,
                history,
                env.state_dict(),
                match_assignments,
                initialization_hash,
            )
            if update % config.snapshot_interval_updates == 0:
                snapshot = args.output_dir / "snapshots" / f"update-{update:06d}.pt"
                _save_progress(
                    snapshot,
                    model,
                    optimizer,
                    config,
                    config_hash,
                    bootstrap_hash,
                    update,
                    environment_steps,
                    seed_index,
                    history,
                    env.state_dict(),
                    match_assignments,
                    initialization_hash,
                )
                _save_inference(
                    args.output_dir / "snapshots" / f"update-{update:06d}-model.pt",
                    model,
                    config,
                    config_hash,
                    bootstrap_hash,
                    initialization_hash,
                    update,
                    environment_steps,
                )
            print(json.dumps(metrics, sort_keys=True), flush=True)
    finally:
        _save_progress(
            progress_path,
            model,
            optimizer,
            config,
            config_hash,
            bootstrap_hash,
            update,
            environment_steps,
            seed_index,
            history,
            env.state_dict(),
            match_assignments,
            initialization_hash,
        )
        _save_inference(
            args.output_dir / "model.pt",
            model,
            config,
            config_hash,
            bootstrap_hash,
            initialization_hash,
            update,
            environment_steps,
        )


def _collect_rollout(
    env: VersusVectorEnv,
    observation: VersusObservation,
    model: VersusActorCritic,
    match_assignments: list[MatchAssignment],
    bootstrap: VersusActorCritic,
    historical: list[OpponentPoolEntry],
    allow_observed: bool,
    config: SelfPlayConfig,
    reward_config: PotentialConfig,
    seed_index: int,
) -> tuple[
    list[DecisionTransition],
    VersusObservation,
    int,
    int,
    dict[str, float | int],
]:
    by_time: list[list[DecisionTransition]] = []
    actor_assignments, model_cache = _resolve_actor_assignments(
        match_assignments, bootstrap, historical, allow_observed
    )
    decision_count = config.parallel_matches * 2
    if len(actor_assignments) != decision_count:
        raise ValueError("opponent assignment count differs from decisions")
    counters = {
        "completed_matches": 0,
        "self_play_completed": 0,
        "bootstrap_games": 0,
        "bootstrap_wins": 0,
        "bootstrap_losses": 0,
        "bootstrap_draws": 0,
        "historical_games": 0,
        "historical_wins": 0,
        "historical_losses": 0,
        "historical_draws": 0,
        "learner_decisions": 0,
        "learner_lines": 0,
        "learner_attack": 0,
        "learner_outgoing_attack": 0,
        "learner_tetrises": 0,
        "learner_t_spin_mini": 0,
        "learner_t_spin_full": 0,
        "learner_perfect_clears": 0,
    }
    entropy_sum = 0.0
    normalized_entropy_sum = 0.0
    max_probability_sum = 0.0
    candidate_count_sum = 0
    for _ in range(config.rollout_steps):
        if bool(torch.any(observation.done)):
            raise RuntimeError("rollout started with an unreset completed match")
        with torch.no_grad():
            values = model.value(observation.state_features)
        selections: list[int] = []
        log_probabilities: list[float] = []
        candidate_views: list[Tensor] = []
        for decision in range(decision_count):
            start, end = observation.offsets[decision : decision + 2]
            actor = actor_assignments[decision] or model
            with torch.no_grad():
                logits = actor.actor_logits(observation.candidate_features[start:end])
            distribution = Categorical(logits=logits)
            action = distribution.sample()
            selected = int(action.item())
            selections.append(selected)
            log_probabilities.append(float(distribution.log_prob(action).item()))
            candidate_views.append(observation.candidate_features[start:end].clone())
            if actor_assignments[decision] is None:
                candidate_count = end - start
                entropy = float(distribution.entropy().item())
                counters["learner_decisions"] += 1
                candidate_count_sum += candidate_count
                entropy_sum += entropy
                if candidate_count > 1:
                    normalized_entropy_sum += entropy / math.log(candidate_count)
                max_probability_sum += float(distribution.probs.max().item())
                diagnostics = observation.candidate_diagnostics[start + selected]
                lines, t_spin, perfect_clear, attack, outgoing = (
                    int(value.item()) for value in diagnostics
                )
                counters["learner_lines"] += lines
                counters["learner_attack"] += attack
                counters["learner_outgoing_attack"] += outgoing
                counters["learner_tetrises"] += int(lines == 4)
                counters["learner_t_spin_mini"] += int(t_spin == 1)
                counters["learner_t_spin_full"] += int(t_spin == 2)
                counters["learner_perfect_clears"] += int(perfect_clear != 0)

        next_observation = env.step(selections)
        terminal = next_observation.done
        with torch.no_grad():
            next_values = model.value(next_observation.state_features)
            next_values = torch.where(terminal, torch.zeros_like(next_values), next_values)
            rewards = transition_reward(
                observation.state_features,
                next_observation.state_features,
                next_observation.results,
                terminal,
                reward_config,
            )
        step_transitions = [
            DecisionTransition(
                candidates=candidate_views[index],
                state=observation.state_features[index].clone(),
                action=selections[index],
                old_log_probability=log_probabilities[index],
                old_value=float(values[index].item()),
                reward=float(rewards[index].item()),
                next_value=float(next_values[index].item()),
                terminal=bool(terminal[index].item()),
                is_learner=actor_assignments[index] is None,
            )
            for index in range(decision_count)
        ]
        by_time.append(step_transitions)

        completed_indices = [
            index for index in range(config.parallel_matches) if bool(terminal[index * 2].item())
        ]
        counters["completed_matches"] += len(completed_indices)
        if completed_indices:
            for match_index in completed_indices:
                assignment = match_assignments[match_index]
                if assignment.kind == "self_play":
                    counters["self_play_completed"] += 1
                    continue
                side = assignment.learner_side
                if side is None:
                    raise RuntimeError("fixed opponent match lacks learner side")
                prefix = assignment.kind
                outcome = int(next_observation.results[match_index * 2 + side].item())
                counters[f"{prefix}_games"] += 1
                counters[f"{prefix}_wins"] += int(outcome > 0)
                counters[f"{prefix}_losses"] += int(outcome < 0)
                counters[f"{prefix}_draws"] += int(outcome == 0)
            reset_seeds = [
                _scheduled_seed(config, seed_index + offset)
                for offset in range(len(completed_indices))
            ]
            seed_index += len(completed_indices)
            next_observation = env.reset_done(reset_seeds)
            for match_index, seed in zip(completed_indices, reset_seeds, strict=True):
                assignment = _new_match_assignment(config, match_index, seed, historical)
                match_assignments[match_index] = assignment
                pair = _actors_for_assignment(assignment, bootstrap, model_cache, allow_observed)
                base = match_index * 2
                actor_assignments[base : base + 2] = pair
        observation = next_observation

    advantages = torch.zeros((config.rollout_steps, decision_count), dtype=torch.float32)
    running = torch.zeros(decision_count, dtype=torch.float32)
    for step in range(config.rollout_steps - 1, -1, -1):
        records = by_time[step]
        rewards = torch.tensor([record.reward for record in records])
        values = torch.tensor([record.old_value for record in records])
        next_values = torch.tensor([record.next_value for record in records])
        terminals = torch.tensor([record.terminal for record in records], dtype=torch.float32)
        delta = rewards + config.gamma * next_values - values
        running = delta + config.gamma * config.gae_lambda * (1.0 - terminals) * running
        advantages[step] = running

    flattened = [record for records in by_time for record in records]
    for record, advantage in zip(flattened, advantages.flatten(), strict=True):
        record.advantage = float(advantage.item())
        record.return_target = record.advantage + record.old_value
    learner_transitions = [record for record in flattened if record.is_learner]
    learner_decisions = counters["learner_decisions"]
    rollout_metrics: dict[str, float | int] = dict(counters)
    if learner_decisions:
        rollout_metrics.update(
            {
                "rollout_entropy": entropy_sum / learner_decisions,
                "rollout_normalized_entropy": normalized_entropy_sum / learner_decisions,
                "rollout_effective_choices": math.exp(entropy_sum / learner_decisions),
                "rollout_mean_max_probability": max_probability_sum / learner_decisions,
                "rollout_mean_candidates": candidate_count_sum / learner_decisions,
                "lines_per_piece": counters["learner_lines"] / learner_decisions,
                "attack_per_piece": counters["learner_attack"] / learner_decisions,
                "outgoing_attack_per_piece": counters["learner_outgoing_attack"]
                / learner_decisions,
                "tetris_per_100": 100.0 * counters["learner_tetrises"] / learner_decisions,
                "t_spin_mini_per_100": 100.0 * counters["learner_t_spin_mini"] / learner_decisions,
                "t_spin_full_per_100": 100.0 * counters["learner_t_spin_full"] / learner_decisions,
                "perfect_clear_per_100": 100.0
                * counters["learner_perfect_clears"]
                / learner_decisions,
            }
        )
    return learner_transitions, observation, seed_index, len(flattened), rollout_metrics


def _ppo_update(
    model: VersusActorCritic,
    optimizer: torch.optim.Optimizer,
    transitions: list[DecisionTransition],
    config: SelfPlayConfig,
    entropy_coefficient: float,
) -> dict[str, float]:
    advantages = torch.tensor([record.advantage for record in transitions])
    advantages = (advantages - advantages.mean()) / advantages.std().clamp_min(1e-6)
    losses: list[float] = []
    policy_losses: list[float] = []
    value_losses: list[float] = []
    entropies: list[float] = []
    normalized_entropies: list[float] = []
    approximate_kls: list[float] = []
    clip_fractions: list[float] = []
    gradient_norms: list[float] = []
    explained_variances: list[float] = []
    for _ in range(config.ppo_epochs):
        order = torch.randperm(len(transitions)).tolist()
        for start in range(0, len(order), config.minibatch_decisions):
            indices = order[start : start + config.minibatch_decisions]
            records = [transitions[index] for index in indices]
            candidate_tensor = torch.cat([record.candidates for record in records], dim=0)
            logits = model.actor_logits(candidate_tensor)
            new_log_probabilities = []
            entropy_values = []
            normalized_entropy_values = []
            offset = 0
            for record in records:
                end = offset + record.candidates.shape[0]
                distribution = Categorical(logits=logits[offset:end])
                action = torch.tensor(record.action)
                new_log_probabilities.append(distribution.log_prob(action))
                candidate_entropy = distribution.entropy()
                entropy_values.append(candidate_entropy)
                candidate_count = record.candidates.shape[0]
                if candidate_count > 1:
                    normalized_entropy_values.append(candidate_entropy / math.log(candidate_count))
                else:
                    normalized_entropy_values.append(torch.zeros_like(candidate_entropy))
                offset = end
            new_log_probability = torch.stack(new_log_probabilities)
            entropy = torch.stack(entropy_values).mean()
            normalized_entropy = torch.stack(normalized_entropy_values).mean()
            old_log_probability = torch.tensor([record.old_log_probability for record in records])
            batch_advantage = advantages[indices]
            log_ratio = new_log_probability - old_log_probability
            ratio = torch.exp(log_ratio)
            unclipped = ratio * batch_advantage
            clipped = (
                torch.clamp(ratio, 1.0 - config.clip_ratio, 1.0 + config.clip_ratio)
                * batch_advantage
            )
            policy_loss = -torch.minimum(unclipped, clipped).mean()
            states = torch.stack([record.state for record in records])
            values = model.value(states)
            targets = torch.tensor([record.return_target for record in records])
            value_loss = torch.nn.functional.mse_loss(values, targets)
            entropy_objective = normalized_entropy if config.normalize_entropy else entropy
            loss = (
                policy_loss
                + config.value_coefficient * value_loss
                - entropy_coefficient * entropy_objective
            )
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            gradient_norm = torch.nn.utils.clip_grad_norm_(model.parameters(), config.max_grad_norm)
            optimizer.step()
            losses.append(float(loss.item()))
            policy_losses.append(float(policy_loss.item()))
            value_losses.append(float(value_loss.item()))
            entropies.append(float(entropy.item()))
            normalized_entropies.append(float(normalized_entropy.item()))
            approximate_kls.append(float(((ratio - 1.0) - log_ratio).mean().item()))
            clip_fractions.append(
                float(((ratio - 1.0).abs() > config.clip_ratio).float().mean().item())
            )
            gradient_norms.append(float(gradient_norm.item()))
            target_variance = targets.var(unbiased=False)
            if float(target_variance.item()) > 1e-12:
                residual_variance = (targets - values).var(unbiased=False)
                explained_variances.append(
                    float((1.0 - residual_variance / target_variance).item())
                )

    return {
        "loss": _mean(losses),
        "policy_loss": _mean(policy_losses),
        "value_loss": _mean(value_losses),
        "ppo_entropy": _mean(entropies),
        "ppo_normalized_entropy": _mean(normalized_entropies),
        "entropy_loss_contribution": -entropy_coefficient
        * _mean(normalized_entropies if config.normalize_entropy else entropies),
        "approximate_kl": _mean(approximate_kls),
        "clip_fraction": _mean(clip_fractions),
        "gradient_norm": _mean(gradient_norms),
        "explained_variance": _mean(explained_variances) if explained_variances else 0.0,
        "mean_reward": _mean([record.reward for record in transitions]),
    }


def _load_opponent_pool(
    output_dir: Path, limit: int, allow_observed: bool
) -> list[OpponentPoolEntry]:
    paths = sorted((output_dir / "snapshots").glob("update-*-model.pt"))[-limit:]
    entries = []
    for path in paths:
        loaded = load_versus_actor(path, allow_observed=allow_observed).model
        loaded.eval()
        for parameter in loaded.parameters():
            parameter.requires_grad_(False)
        entries.append(OpponentPoolEntry(checkpoint=str(path), model=loaded))
    return entries


def _new_match_assignment(
    config: SelfPlayConfig,
    match_index: int,
    seed: int,
    historical: list[OpponentPoolEntry],
) -> MatchAssignment:
    self_play_matches = round(config.parallel_matches * config.self_play_fraction)
    historical_matches = round(config.parallel_matches * config.historical_fraction)
    if match_index < self_play_matches:
        return MatchAssignment(kind="self_play", learner_side=None)
    learner_side = seed % 2
    if match_index < self_play_matches + historical_matches and historical:
        opponent = historical[seed % len(historical)]
        return MatchAssignment(
            kind="historical",
            learner_side=learner_side,
            opponent_checkpoint=opponent.checkpoint,
        )
    return MatchAssignment(kind="bootstrap", learner_side=learner_side)


def _resolve_actor_assignments(
    assignments: list[MatchAssignment],
    bootstrap: VersusActorCritic,
    historical: list[OpponentPoolEntry],
    allow_observed: bool,
) -> tuple[list[VersusActorCritic | None], dict[str, VersusActorCritic]]:
    cache = {entry.checkpoint: entry.model for entry in historical}
    actors: list[VersusActorCritic | None] = []
    for assignment in assignments:
        assignment.validate()
        actors.extend(_actors_for_assignment(assignment, bootstrap, cache, allow_observed))
    return actors, cache


def _actors_for_assignment(
    assignment: MatchAssignment,
    bootstrap: VersusActorCritic,
    cache: dict[str, VersusActorCritic],
    allow_observed: bool,
) -> list[VersusActorCritic | None]:
    if assignment.kind == "self_play":
        return [None, None]
    if assignment.kind == "bootstrap":
        opponent = bootstrap
    else:
        checkpoint = assignment.opponent_checkpoint
        if checkpoint is None:
            raise ValueError("historical assignment lacks checkpoint")
        if checkpoint not in cache:
            loaded = load_versus_actor(Path(checkpoint), allow_observed=allow_observed).model
            loaded.eval()
            for parameter in loaded.parameters():
                parameter.requires_grad_(False)
            cache[checkpoint] = loaded
        opponent = cache[checkpoint]
    pair: list[VersusActorCritic | None] = [opponent, opponent]
    if assignment.learner_side is None:
        raise ValueError("fixed-opponent assignment lacks learner side")
    pair[assignment.learner_side] = None
    return pair


def _entropy_coefficient(config: SelfPlayConfig, update: int) -> float:
    final = config.entropy_coefficient_final
    if final is None:
        return config.entropy_coefficient
    fraction = min(max(update, 0) / config.entropy_decay_updates, 1.0)
    return config.entropy_coefficient + fraction * (final - config.entropy_coefficient)


def _add_rolling_scores(
    metrics: dict[str, float | int | str | None],
    history: list[dict[str, float | int | str | None]],
) -> None:
    window = [*history[-9:], metrics]
    for prefix in ("bootstrap", "historical"):
        games = sum(int(row.get(f"{prefix}_games", 0) or 0) for row in window)
        wins = sum(int(row.get(f"{prefix}_wins", 0) or 0) for row in window)
        draws = sum(int(row.get(f"{prefix}_draws", 0) or 0) for row in window)
        metrics[f"{prefix}_rolling_10_games"] = games
        metrics[f"{prefix}_rolling_10_score"] = (wins + 0.5 * draws) / games if games else None


def _save_progress(
    path: Path,
    model: VersusActorCritic,
    optimizer: torch.optim.Optimizer,
    config: SelfPlayConfig,
    config_hash: str,
    bootstrap_hash: str,
    update: int,
    environment_steps: int,
    seed_index: int,
    history: list[dict[str, float | int | str | None]],
    environment_state: dict[str, object],
    match_assignments: list[MatchAssignment],
    initialization_hash: str | None,
) -> None:
    payload = {
        "checkpoint_schema": CHECKPOINT_SCHEMA,
        "config": asdict(config),
        "config_sha256": config_hash,
        "bootstrap_sha256": bootstrap_hash,
        "initialization_sha256": initialization_hash,
        "model_state": model.state_dict(),
        "optimizer_state": optimizer.state_dict(),
        "update": update,
        "environment_steps": environment_steps,
        "seed_index": seed_index,
        "torch_rng_state": torch.get_rng_state(),
        "python_rng_state": random.getstate(),
        "history": history,
        "environment_state": environment_state,
        "match_assignments": [asdict(assignment) for assignment in match_assignments],
    }
    _atomic_torch_save(payload, path)


def _save_inference(
    path: Path,
    model: VersusActorCritic,
    config: SelfPlayConfig,
    config_hash: str,
    bootstrap_hash: str,
    initialization_hash: str | None,
    update: int,
    environment_steps: int,
) -> None:
    _atomic_torch_save(
        {
            "checkpoint_schema": "versus-actor-critic-v1",
            "mechanics_status": MECHANICS_STATUS,
            "config": asdict(config),
            "config_sha256": config_hash,
            "bootstrap_sha256": bootstrap_hash,
            "initialization_sha256": initialization_hash,
            "model_state": model.state_dict(),
            "model_config": asdict(model.config),
            "solo_model_config": model.solo_model.config.to_dict(),
            "update": update,
            "environment_steps": environment_steps,
        },
        path,
    )


def _validate_resume(
    payload: dict[str, object],
    config: SelfPlayConfig,
    config_hash: str,
    bootstrap_hash: str,
) -> None:
    if payload.get("checkpoint_schema") != CHECKPOINT_SCHEMA:
        raise ValueError("incompatible self-play checkpoint schema")
    if payload.get("config") != asdict(config) or payload.get("config_sha256") != config_hash:
        raise ValueError("self-play config changed; refusing semantic resume")
    if payload.get("bootstrap_sha256") != bootstrap_hash:
        raise ValueError("solo bootstrap changed; refusing resume")
    if "environment_state" not in payload:
        raise ValueError("progress checkpoint lacks exact environment state")
    if "match_assignments" not in payload:
        raise ValueError("progress checkpoint lacks persistent opponent assignments")


def _atomic_torch_save(payload: dict[str, object], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    torch.save(payload, temporary)
    os.replace(temporary, path)


def _scheduled_seed(config: SelfPlayConfig, index: int) -> int:
    return config.base_seed + config.seed_stride * index


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _mean(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0


def _as_random_state(value: object) -> tuple:
    if not isinstance(value, tuple):
        raise ValueError("invalid Python RNG state")
    return value


if __name__ == "__main__":
    main()
