import argparse
import hashlib
import json
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

CHECKPOINT_SCHEMA = "versus-selfplay-ppo-progress-v1"


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

    @classmethod
    def load(cls, path: Path) -> "SelfPlayConfig":
        payload = json.loads(path.read_text(encoding="utf-8"))
        config = cls(**payload)
        config.validate()
        return config

    def validate(self) -> None:
        if self.schema_version != "versus-selfplay-ppo-v1":
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
    parser.add_argument("--allow-observed", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.hours <= 0.0 or args.threads <= 0 or args.max_updates < 0:
        raise ValueError("hours/threads must be positive and max-updates nonnegative")
    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    config = SelfPlayConfig.load(args.config)
    solo = load_scorer(args.bootstrap, allow_observed=args.allow_observed)
    model = VersusActorCritic(solo)
    bootstrap_opponent = VersusActorCritic(solo)
    bootstrap_opponent.eval()
    for parameter in bootstrap_opponent.parameters():
        parameter.requires_grad_(False)
    optimizer = torch.optim.Adam(model.parameters(), lr=config.learning_rate)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    progress_path = args.output_dir / "latest.pt"
    config_hash = _sha256_file(args.config)
    bootstrap_hash = _sha256_file(args.bootstrap)
    update = 0
    environment_steps = 0
    seed_index = 0
    history: list[dict[str, float | int]] = []

    if args.resume and progress_path.exists():
        payload = torch.load(progress_path, map_location="cpu", weights_only=True)
        _validate_resume(payload, config, config_hash, bootstrap_hash)
        model.load_state_dict(payload["model_state"], strict=True)
        optimizer.load_state_dict(payload["optimizer_state"])
        update = int(payload["update"])
        environment_steps = int(payload["environment_steps"])
        seed_index = int(payload["seed_index"])
        history = list(payload.get("history", []))
        torch.set_rng_state(payload["torch_rng_state"])
        random.setstate(_as_random_state(payload["python_rng_state"]))
    else:
        torch.manual_seed(config.base_seed)
        random.seed(config.base_seed)

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
            try:
                batch_seeds = [
                    _scheduled_seed(config, seed_index + offset)
                    for offset in range(config.parallel_matches)
                ]
                seed_index += config.parallel_matches
                env = VersusVectorEnv(batch_seeds, config.frames_per_placement)
                historical = _load_opponent_pool(
                    args.output_dir, config.opponent_pool_limit, args.allow_observed
                )
                assignments = _opponent_assignments(config, update, bootstrap_opponent, historical)
                transitions, completed, seed_index, simulated_decisions = _collect_rollout(
                    env,
                    env.observe(),
                    model,
                    assignments,
                    config,
                    reward_config,
                    seed_index,
                )
                metrics = _ppo_update(model, optimizer, transitions, config)
            except BaseException:
                model.load_state_dict(model_before, strict=True)
                optimizer.load_state_dict(optimizer_before)
                torch.set_rng_state(torch_rng_before)
                random.setstate(python_rng_before)
                seed_index = seed_index_before
                raise
            update += 1
            environment_steps += simulated_decisions
            metrics.update(
                {
                    "update": update,
                    "environment_steps": environment_steps,
                    "completed_matches": completed,
                    "seconds": time.monotonic() - started,
                }
            )
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
                )
                _save_inference(
                    args.output_dir / "snapshots" / f"update-{update:06d}-model.pt",
                    model,
                    config,
                    config_hash,
                    bootstrap_hash,
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
        )
        _save_inference(
            args.output_dir / "model.pt",
            model,
            config,
            config_hash,
            bootstrap_hash,
            update,
            environment_steps,
        )


def _collect_rollout(
    env: VersusVectorEnv,
    observation: VersusObservation,
    model: VersusActorCritic,
    actor_assignments: list[VersusActorCritic | None],
    config: SelfPlayConfig,
    reward_config: PotentialConfig,
    seed_index: int,
) -> tuple[list[DecisionTransition], int, int, int]:
    by_time: list[list[DecisionTransition]] = []
    completed_matches = 0
    decision_count = config.parallel_matches * 2
    if len(actor_assignments) != decision_count:
        raise ValueError("opponent assignment count differs from decisions")
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
            selections.append(int(action.item()))
            log_probabilities.append(float(distribution.log_prob(action).item()))
            candidate_views.append(observation.candidate_features[start:end].clone())

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
            )
            for index in range(decision_count)
        ]
        by_time.append(step_transitions)

        completed = int(terminal[0::2].sum().item())
        completed_matches += completed
        if completed:
            reset_seeds = [
                _scheduled_seed(config, seed_index + offset) for offset in range(completed)
            ]
            seed_index += completed
            next_observation = env.reset_done(reset_seeds)
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
    learner_transitions = [
        record
        for index, record in enumerate(flattened)
        if actor_assignments[index % decision_count] is None
    ]
    return learner_transitions, completed_matches, seed_index, len(flattened)


def _ppo_update(
    model: VersusActorCritic,
    optimizer: torch.optim.Optimizer,
    transitions: list[DecisionTransition],
    config: SelfPlayConfig,
) -> dict[str, float]:
    advantages = torch.tensor([record.advantage for record in transitions])
    advantages = (advantages - advantages.mean()) / advantages.std().clamp_min(1e-6)
    losses: list[float] = []
    policy_losses: list[float] = []
    value_losses: list[float] = []
    entropies: list[float] = []
    for _ in range(config.ppo_epochs):
        order = torch.randperm(len(transitions)).tolist()
        for start in range(0, len(order), config.minibatch_decisions):
            indices = order[start : start + config.minibatch_decisions]
            records = [transitions[index] for index in indices]
            candidate_tensor = torch.cat([record.candidates for record in records], dim=0)
            logits = model.actor_logits(candidate_tensor)
            new_log_probabilities = []
            entropy_values = []
            offset = 0
            for record in records:
                end = offset + record.candidates.shape[0]
                distribution = Categorical(logits=logits[offset:end])
                action = torch.tensor(record.action)
                new_log_probabilities.append(distribution.log_prob(action))
                entropy_values.append(distribution.entropy())
                offset = end
            new_log_probability = torch.stack(new_log_probabilities)
            entropy = torch.stack(entropy_values).mean()
            old_log_probability = torch.tensor([record.old_log_probability for record in records])
            batch_advantage = advantages[indices]
            ratio = torch.exp(new_log_probability - old_log_probability)
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
            loss = (
                policy_loss
                + config.value_coefficient * value_loss
                - config.entropy_coefficient * entropy
            )
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), config.max_grad_norm)
            optimizer.step()
            losses.append(float(loss.item()))
            policy_losses.append(float(policy_loss.item()))
            value_losses.append(float(value_loss.item()))
            entropies.append(float(entropy.item()))

    return {
        "loss": _mean(losses),
        "policy_loss": _mean(policy_losses),
        "value_loss": _mean(value_losses),
        "entropy": _mean(entropies),
        "mean_reward": _mean([record.reward for record in transitions]),
    }


def _load_opponent_pool(
    output_dir: Path, limit: int, allow_observed: bool
) -> list[VersusActorCritic]:
    paths = sorted((output_dir / "snapshots").glob("update-*-model.pt"))[-limit:]
    models = []
    for path in paths:
        loaded = load_versus_actor(path, allow_observed=allow_observed).model
        loaded.eval()
        for parameter in loaded.parameters():
            parameter.requires_grad_(False)
        models.append(loaded)
    return models


def _opponent_assignments(
    config: SelfPlayConfig,
    update: int,
    bootstrap: VersusActorCritic,
    historical: list[VersusActorCritic],
) -> list[VersusActorCritic | None]:
    self_play_matches = round(config.parallel_matches * config.self_play_fraction)
    historical_matches = round(config.parallel_matches * config.historical_fraction)
    assignments: list[VersusActorCritic | None] = []
    for match_index in range(config.parallel_matches):
        if match_index < self_play_matches:
            assignments.extend((None, None))
            continue
        learner_side = (match_index + update) % 2
        if match_index < self_play_matches + historical_matches and historical:
            opponent = historical[(match_index + update) % len(historical)]
        else:
            opponent = bootstrap
        pair: list[VersusActorCritic | None] = [opponent, opponent]
        pair[learner_side] = None
        assignments.extend(pair)
    return assignments


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
    history: list[dict[str, float | int]],
) -> None:
    payload = {
        "checkpoint_schema": CHECKPOINT_SCHEMA,
        "config": asdict(config),
        "config_sha256": config_hash,
        "bootstrap_sha256": bootstrap_hash,
        "model_state": model.state_dict(),
        "optimizer_state": optimizer.state_dict(),
        "update": update,
        "environment_steps": environment_steps,
        "seed_index": seed_index,
        "torch_rng_state": torch.get_rng_state(),
        "python_rng_state": random.getstate(),
        "history": history,
    }
    _atomic_torch_save(payload, path)


def _save_inference(
    path: Path,
    model: VersusActorCritic,
    config: SelfPlayConfig,
    config_hash: str,
    bootstrap_hash: str,
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
