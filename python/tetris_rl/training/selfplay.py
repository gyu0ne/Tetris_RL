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
from tetris_rl.models import (
    VersusActorCritic,
    VersusModelConfig,
    load_scorer,
    load_versus_actor,
)
from tetris_rl.training.opponent_pool import OpponentPoolState
from tetris_rl.training.reward import (
    POTENTIAL_COMPONENT_NAMES,
    PotentialConfig,
    tactical_candidate_scores,
    tactical_potential_components,
    transition_reward_details,
)

CHECKPOINT_SCHEMA = "versus-selfplay-ppo-progress-v3"
CHECKPOINT_SCHEMA_V5 = "versus-selfplay-ppo-progress-v4"
CHECKPOINT_SCHEMA_V6 = "versus-selfplay-ppo-progress-v5"


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
    model_architecture: str = "legacy-additive-v1"
    solo_learning_rate_multiplier: float = 1.0
    kickstart_coefficient: float = 0.0
    kickstart_coefficient_final: float = 0.0
    kickstart_decay_updates: int = 1
    pfsp_exponent: float = 1.0
    pfsp_min_weight: float = 0.05
    anchor_checkpoints: list[str] | None = None
    tactical_potential_fraction: float = 0.0
    tactical_curriculum_coefficient: float = 0.0
    tactical_curriculum_coefficient_final: float = 0.0
    tactical_curriculum_decay_updates: int = 1
    tactical_curriculum_temperature: float = 1.0
    pool_promotion_interval_updates: int = 50
    opponent_recent_slots: int = 12
    opponent_score_half_life_updates: float = 100.0
    opponent_result_history_limit: int = 256
    historical_balanced_fraction: float = 0.4
    historical_hard_fraction: float = 0.3
    historical_uniform_fraction: float = 0.3
    value_learning_rate_multiplier: float = 1.0
    value_extra_epochs: int = 0
    offense_reward_coefficient: float = 0.0
    offense_reward_coefficient_final: float = 0.0
    offense_reward_hold_updates: int = 0
    offense_reward_decay_updates: int = 1
    offense_reward_attack_scale: float = 4.0

    @classmethod
    def load(cls, path: Path) -> "SelfPlayConfig":
        payload = json.loads(path.read_text(encoding="utf-8"))
        config = cls(**payload)
        config.validate()
        return config

    def validate(self) -> None:
        if self.schema_version not in {
            "versus-selfplay-ppo-v1",
            "versus-selfplay-ppo-v2",
            "versus-selfplay-ppo-v3",
            "versus-selfplay-ppo-v4",
            "versus-selfplay-ppo-v5",
            "versus-selfplay-ppo-v6",
        }:
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
        if self.schema_version in {
            "versus-selfplay-ppo-v3",
            "versus-selfplay-ppo-v4",
            "versus-selfplay-ppo-v5",
            "versus-selfplay-ppo-v6",
        }:
            if self.model_architecture != "joint-residual-v2":
                raise ValueError("v3 requires the joint residual model")
            if self.entropy_coefficient_final is None or self.entropy_coefficient_final < 0.0:
                raise ValueError("v3 requires a nonnegative final entropy coefficient")
            if self.entropy_decay_updates <= 0 or not self.normalize_entropy:
                raise ValueError("v3 requires normalized entropy and positive entropy decay")
            if not 0.0 < self.solo_learning_rate_multiplier <= 1.0:
                raise ValueError("solo learning-rate multiplier must be in (0, 1]")
            if min(self.kickstart_coefficient, self.kickstart_coefficient_final) < 0.0:
                raise ValueError("kickstart coefficients must be nonnegative")
            if self.kickstart_decay_updates <= 0:
                raise ValueError("kickstart decay must be positive")
            if self.pfsp_exponent <= 0.0 or self.pfsp_min_weight <= 0.0:
                raise ValueError("PFSP settings must be positive")
        if self.schema_version in {
            "versus-selfplay-ppo-v4",
            "versus-selfplay-ppo-v5",
            "versus-selfplay-ppo-v6",
        }:
            if not 0.0 <= self.tactical_potential_fraction <= 1.0:
                raise ValueError("tactical potential fraction must be in [0, 1]")
            if (
                min(
                    self.tactical_curriculum_coefficient,
                    self.tactical_curriculum_coefficient_final,
                )
                < 0.0
            ):
                raise ValueError("tactical curriculum coefficients must be nonnegative")
            if self.tactical_curriculum_decay_updates <= 0:
                raise ValueError("tactical curriculum decay must be positive")
            if self.tactical_curriculum_temperature <= 0.0:
                raise ValueError("tactical curriculum temperature must be positive")
        if self.schema_version in {"versus-selfplay-ppo-v5", "versus-selfplay-ppo-v6"}:
            if self.pool_promotion_interval_updates <= 0:
                raise ValueError("pool promotion interval must be positive")
            if self.pool_promotion_interval_updates % self.snapshot_interval_updates != 0:
                raise ValueError("pool promotions must align with snapshot updates")
            if not 0 < self.opponent_recent_slots < self.opponent_pool_limit:
                raise ValueError("recent opponent slots must fit inside the pool")
            if self.opponent_score_half_life_updates <= 0.0:
                raise ValueError("opponent score half-life must be positive")
            if self.opponent_result_history_limit <= 0:
                raise ValueError("opponent result history limit must be positive")
            fractions = (
                self.historical_balanced_fraction,
                self.historical_hard_fraction,
                self.historical_uniform_fraction,
            )
            if any(fraction < 0.0 for fraction in fractions) or not math.isclose(
                sum(fractions), 1.0, abs_tol=1e-9
            ):
                raise ValueError("historical sampling fractions must be nonnegative and sum to one")
            if self.value_learning_rate_multiplier <= 0.0 or self.value_extra_epochs < 0:
                raise ValueError("invalid value-learning settings")
        if self.schema_version == "versus-selfplay-ppo-v6":
            if not (
                0.0 <= self.offense_reward_coefficient_final <= self.offense_reward_coefficient
            ):
                raise ValueError("offense reward must decay between nonnegative coefficients")
            if (
                self.offense_reward_hold_updates < 0
                or self.offense_reward_decay_updates <= self.offense_reward_hold_updates
            ):
                raise ValueError("offense reward decay must end after its nonnegative hold")
            if self.offense_reward_attack_scale <= 0.0:
                raise ValueError("offense reward attack scale must be positive")


@dataclass(frozen=True)
class OpponentPoolEntry:
    checkpoint: str
    model: VersusActorCritic


@dataclass(frozen=True)
class MatchAssignment:
    kind: str
    learner_side: int | None
    opponent_checkpoint: str | None = None
    sampling_mode: str | None = None

    def validate(self) -> None:
        if self.kind == "self_play":
            if (
                self.learner_side is not None
                or self.opponent_checkpoint is not None
                or self.sampling_mode is not None
            ):
                raise ValueError("self-play assignment cannot name one learner side or opponent")
            return
        if self.kind not in {"bootstrap", "historical"} or self.learner_side not in {0, 1}:
            raise ValueError("invalid fixed-opponent assignment")
        if (self.kind == "historical") != (self.opponent_checkpoint is not None):
            raise ValueError("historical assignment checkpoint mismatch")
        if self.kind == "historical" and self.sampling_mode not in {
            None,
            "legacy",
            "balanced",
            "hard",
            "uniform",
        }:
            raise ValueError("invalid historical sampling mode")
        if self.kind == "bootstrap" and self.sampling_mode is not None:
            raise ValueError("bootstrap assignment cannot name a historical sampling mode")


@dataclass
class DecisionTransition:
    candidates: Tensor
    candidate_diagnostics: Tensor
    state: Tensor
    action: int
    old_log_probability: float
    old_value: float
    reward: float
    terminal_reward: float
    shaping_reward: float
    offense_reward: float
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
    architecture_version = 2 if config.model_architecture == "joint-residual-v2" else 1
    model_config = VersusModelConfig(architecture_version=architecture_version)
    model = VersusActorCritic(solo, model_config)
    bootstrap_opponent = VersusActorCritic(solo, model_config)
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
        if initialized.model.config.architecture_version != architecture_version:
            raise ValueError("initialization checkpoint architecture differs from training config")
        model.load_state_dict(initialized.model.state_dict(), strict=True)
        initialization_hash = _sha256_file(args.initialize_from)
    optimizer = _build_optimizer(model, config)
    update = 0
    environment_steps = 0
    seed_index = 0
    history: list[dict[str, float | int | str | None]] = []
    league_stats: dict[str, dict[str, int]] = {}
    pool_state: OpponentPoolState | None = None
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
        league_stats = {
            str(checkpoint): {str(key): int(value) for key, value in stats.items()}
            for checkpoint, stats in payload.get("league_stats", {}).items()
        }
        if config.schema_version in {"versus-selfplay-ppo-v5", "versus-selfplay-ppo-v6"}:
            raw_pool_state = payload.get("opponent_pool_state")
            if not isinstance(raw_pool_state, dict):
                raise ValueError("v5 resume checkpoint lacks stable opponent-pool state")
            pool_state = OpponentPoolState.from_payload(raw_pool_state)
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
        if config.schema_version in {"versus-selfplay-ppo-v5", "versus-selfplay-ppo-v6"}:
            anchors = list(config.anchor_checkpoints or [])
            if args.initialize_from is not None:
                anchors.insert(0, str(args.initialize_from))
            pool_state = OpponentPoolState.initialize(anchors)
            if len(pool_state.members) > config.opponent_pool_limit:
                raise ValueError("initial anchors exceed opponent-pool limit")
        historical = _load_opponent_pool(args.output_dir, config, args.allow_observed, pool_state)
        match_assignments = [
            _new_match_assignment(
                config,
                index,
                seed,
                historical,
                league_stats,
                pool_state=pool_state,
                update=update,
            )
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
    reward_config = PotentialConfig(
        gamma=config.gamma,
        shaping_scale=config.shaping_scale,
        tactical_fraction=config.tactical_potential_fraction,
    )
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
            league_stats_before = deepcopy(league_stats)
            pool_state_before = deepcopy(pool_state)
            try:
                historical = _load_opponent_pool(
                    args.output_dir, config, args.allow_observed, pool_state
                )
                rollout_started = time.monotonic()
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
                        league_stats,
                        pool_state,
                        update,
                    )
                )
                rollout_seconds = time.monotonic() - rollout_started
                entropy_coefficient = _entropy_coefficient(config, update)
                kickstart_coefficient = _kickstart_coefficient(config, update)
                tactical_coefficient = _tactical_curriculum_coefficient(config, update)
                ppo_started = time.monotonic()
                metrics = _ppo_update(
                    model,
                    bootstrap_opponent,
                    optimizer,
                    transitions,
                    config,
                    entropy_coefficient,
                    kickstart_coefficient,
                    tactical_coefficient,
                    reward_config,
                )
                ppo_seconds = time.monotonic() - ppo_started
                metrics.update(rollout_metrics)
                metrics.update(
                    {
                        "rollout_total_seconds": rollout_seconds,
                        "rollout_simulated_decisions_per_second": (
                            simulated_decisions / rollout_seconds
                        ),
                        "ppo_seconds": ppo_seconds,
                    }
                )
            except BaseException:
                model.load_state_dict(model_before, strict=True)
                optimizer.load_state_dict(optimizer_before)
                torch.set_rng_state(torch_rng_before)
                random.setstate(python_rng_before)
                seed_index = seed_index_before
                match_assignments = assignments_before
                league_stats = league_stats_before
                pool_state = pool_state_before
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
                    "kickstart_coefficient": kickstart_coefficient,
                    "offense_reward_coefficient": _offense_reward_coefficient(config, update - 1),
                    "tactical_curriculum_coefficient": tactical_coefficient,
                    "seconds": time.monotonic() - started,
                }
            )
            _add_rolling_scores(metrics, history)
            history.append(metrics)
            history = history[-100:]
            pool_event: dict[str, object] | None = None
            if pool_state is not None and update % config.pool_promotion_interval_updates == 0:
                promoted_path = args.output_dir / "snapshots" / f"update-{update:06d}-model.pt"
                _save_inference(
                    promoted_path,
                    model,
                    config,
                    config_hash,
                    bootstrap_hash,
                    initialization_hash,
                    update,
                    environment_steps,
                )
                pool_event = pool_state.promote(
                    str(promoted_path),
                    update,
                    limit=config.opponent_pool_limit,
                    recent_slots=config.opponent_recent_slots,
                )
            if pool_state is not None:
                metrics.update(_opponent_pool_metrics(pool_state, config, update))
                if pool_event is not None:
                    metrics.update(
                        {
                            "opponent_pool_added": pool_event["added"],
                            "opponent_pool_removed": pool_event["removed"],
                            "opponent_pool_demoted": pool_event["demoted"],
                        }
                    )
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
                league_stats,
                pool_state,
            )
            _save_inference(
                args.output_dir / "latest-model.pt",
                model,
                config,
                config_hash,
                bootstrap_hash,
                initialization_hash,
                update,
                environment_steps,
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
                    league_stats,
                    pool_state,
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
            league_stats,
            pool_state,
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
    league_stats: dict[str, dict[str, int]],
    pool_state: OpponentPoolState | None = None,
    update: int = 0,
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
        "historical_balanced_games": 0,
        "historical_hard_games": 0,
        "historical_uniform_games": 0,
        "learner_decisions": 0,
        "learner_lines": 0,
        "learner_attack": 0,
        "learner_outgoing_attack": 0,
        "learner_tetrises": 0,
        "learner_t_spin_mini": 0,
        "learner_t_spin_full": 0,
        "learner_perfect_clears": 0,
        "learner_cancelled_attack": 0,
        "learner_max_height_sum": 0,
        "learner_holes_sum": 0,
        "learner_pending_sum": 0,
        "learner_ready_sum": 0,
        "learner_danger_decisions": 0,
        "learner_terminal_decisions": 0,
        "learner_terminal_wins": 0,
        "learner_terminal_losses": 0,
        "learner_terminal_draws": 0,
    }
    entropy_sum = 0.0
    normalized_entropy_sum = 0.0
    max_probability_sum = 0.0
    candidate_count_sum = 0
    all_candidate_count_sum = 0
    environment_seconds = 0.0
    policy_seconds = 0.0
    value_reward_seconds = 0.0
    shaping_absolute_sum = 0.0
    shaping_nonzero = 0
    shaping_max_absolute = 0.0
    shaping_component_absolute_sums = torch.zeros(len(POTENTIAL_COMPONENT_NAMES))
    shaping_component_signed_sums = torch.zeros(len(POTENTIAL_COMPONENT_NAMES))
    offense_coefficient = _offense_reward_coefficient(config, update)
    offense_reward_sum = 0.0
    offense_reward_absolute_sum = 0.0
    offense_reward_nonzero = 0
    offense_reward_max_absolute = 0.0
    for _ in range(config.rollout_steps):
        if bool(torch.any(observation.done)):
            raise RuntimeError("rollout started with an unreset completed match")
        value_started = time.monotonic()
        with torch.no_grad():
            values = model.value(observation.state_features)
            current_tactical = tactical_potential_components(
                observation.candidate_diagnostics,
                observation.offsets,
                reward_config,
            )
        value_reward_seconds += time.monotonic() - value_started
        policy_started = time.monotonic()
        logits_by_decision = _batched_actor_logits(observation, actor_assignments, model)
        selections: list[int] = []
        log_probabilities: list[float] = []
        candidate_views: list[Tensor] = []
        diagnostic_views: list[Tensor] = []
        selected_outgoing: list[float] = []
        for decision in range(decision_count):
            start, end = observation.offsets[decision : decision + 2]
            logits = logits_by_decision[decision]
            distribution = Categorical(logits=logits)
            action = distribution.sample()
            selected = int(action.item())
            selections.append(selected)
            log_probabilities.append(float(distribution.log_prob(action).item()))
            candidate_views.append(observation.candidate_features[start:end].clone())
            diagnostic_views.append(observation.candidate_diagnostics[start:end].clone())
            selected_diagnostics = observation.candidate_diagnostics[start + selected]
            selected_outgoing.append(float(selected_diagnostics[4].item()))
            if actor_assignments[decision] is None:
                candidate_count = end - start
                entropy = float(distribution.entropy().item())
                counters["learner_decisions"] += 1
                candidate_count_sum += candidate_count
                entropy_sum += entropy
                if candidate_count > 1:
                    normalized_entropy_sum += entropy / math.log(candidate_count)
                max_probability_sum += float(distribution.probs.max().item())
                diagnostics = selected_diagnostics
                lines, t_spin, perfect_clear, attack, outgoing = (
                    int(value.item()) for value in diagnostics
                )
                counters["learner_lines"] += lines
                counters["learner_attack"] += attack
                counters["learner_outgoing_attack"] += outgoing
                counters["learner_cancelled_attack"] += attack - outgoing
                counters["learner_tetrises"] += int(lines == 4)
                counters["learner_t_spin_mini"] += int(t_spin == 1)
                counters["learner_t_spin_full"] += int(t_spin == 2)
                counters["learner_perfect_clears"] += int(perfect_clear != 0)
                state = observation.state_features[decision]
                max_height = int(state[0].item())
                counters["learner_max_height_sum"] += max_height
                counters["learner_holes_sum"] += int(state[2].item())
                counters["learner_pending_sum"] += int(state[4].item())
                counters["learner_ready_sum"] += int(state[6].item())
                counters["learner_danger_decisions"] += int(max_height >= 16)
        all_candidate_count_sum += observation.candidate_features.shape[0]
        policy_seconds += time.monotonic() - policy_started
        offense_rewards = _net_offense_rewards(
            torch.tensor(selected_outgoing, dtype=torch.float32),
            offense_coefficient,
            config.offense_reward_attack_scale,
        )

        environment_started = time.monotonic()
        next_observation = env.step(selections)
        environment_seconds += time.monotonic() - environment_started
        terminal = next_observation.done
        value_started = time.monotonic()
        with torch.no_grad():
            next_values = model.value(next_observation.state_features)
            next_values = torch.where(terminal, torch.zeros_like(next_values), next_values)
            next_tactical = tactical_potential_components(
                next_observation.candidate_diagnostics,
                next_observation.offsets,
                reward_config,
            )
            rewards, shaping_components = transition_reward_details(
                observation.state_features,
                next_observation.state_features,
                next_observation.results,
                terminal,
                reward_config,
                current_tactical=current_tactical,
                next_tactical=next_tactical,
            )
            rewards = rewards + offense_rewards
        value_reward_seconds += time.monotonic() - value_started
        for index, assignment in enumerate(actor_assignments):
            if assignment is not None:
                continue
            dense = shaping_components[index]
            dense_absolute = abs(float(dense.sum().item()))
            shaping_absolute_sum += dense_absolute
            shaping_nonzero += int(dense_absolute > 1e-12)
            shaping_max_absolute = max(shaping_max_absolute, dense_absolute)
            shaping_component_absolute_sums += dense.abs()
            shaping_component_signed_sums += dense
            offense = float(offense_rewards[index].item())
            offense_reward_sum += offense
            offense_reward_absolute_sum += abs(offense)
            offense_reward_nonzero += int(abs(offense) > 1e-12)
            offense_reward_max_absolute = max(offense_reward_max_absolute, abs(offense))
            if bool(terminal[index].item()):
                outcome = int(next_observation.results[index].item())
                counters["learner_terminal_decisions"] += 1
                counters["learner_terminal_wins"] += int(outcome > 0)
                counters["learner_terminal_losses"] += int(outcome < 0)
                counters["learner_terminal_draws"] += int(outcome == 0)
        step_transitions = [
            DecisionTransition(
                candidates=candidate_views[index],
                candidate_diagnostics=diagnostic_views[index],
                state=observation.state_features[index].clone(),
                action=selections[index],
                old_log_probability=log_probabilities[index],
                old_value=float(values[index].item()),
                reward=float(rewards[index].item()),
                terminal_reward=float(next_observation.results[index].item()),
                shaping_reward=float(shaping_components[index].sum().item()),
                offense_reward=float(offense_rewards[index].item()),
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
                if assignment.kind == "historical":
                    checkpoint = assignment.opponent_checkpoint
                    if checkpoint is None:
                        raise RuntimeError("historical match lacks checkpoint")
                    stats = league_stats.setdefault(
                        checkpoint, {"games": 0, "wins": 0, "losses": 0, "draws": 0}
                    )
                    stats["games"] += 1
                    stats["wins"] += int(outcome > 0)
                    stats["losses"] += int(outcome < 0)
                    stats["draws"] += int(outcome == 0)
                    if pool_state is not None:
                        pool_state.record_result(
                            checkpoint,
                            update,
                            1.0 if outcome > 0 else 0.5 if outcome == 0 else 0.0,
                            history_limit=config.opponent_result_history_limit,
                        )
                        mode = assignment.sampling_mode or "uniform"
                        counters[f"historical_{mode}_games"] += 1
            reset_seeds = [
                _scheduled_seed(config, seed_index + offset)
                for offset in range(len(completed_indices))
            ]
            seed_index += len(completed_indices)
            environment_started = time.monotonic()
            next_observation = env.reset_done(reset_seeds)
            environment_seconds += time.monotonic() - environment_started
            for match_index, seed in zip(completed_indices, reset_seeds, strict=True):
                assignment = _new_match_assignment(
                    config,
                    match_index,
                    seed,
                    historical,
                    league_stats,
                    pool_state=pool_state,
                    update=update,
                )
                match_assignments[match_index] = assignment
                pair = _actors_for_assignment(assignment, bootstrap, model_cache, allow_observed)
                base = match_index * 2
                actor_assignments[base : base + 2] = pair
        observation = next_observation

    advantages = torch.zeros((config.rollout_steps, decision_count), dtype=torch.float32)
    terminal_traces = torch.zeros_like(advantages)
    shaping_traces = torch.zeros_like(advantages)
    offense_traces = torch.zeros_like(advantages)
    running = torch.zeros(decision_count, dtype=torch.float32)
    running_terminal = torch.zeros(decision_count, dtype=torch.float32)
    running_shaping = torch.zeros(decision_count, dtype=torch.float32)
    running_offense = torch.zeros(decision_count, dtype=torch.float32)
    for step in range(config.rollout_steps - 1, -1, -1):
        records = by_time[step]
        rewards = torch.tensor([record.reward for record in records])
        values = torch.tensor([record.old_value for record in records])
        next_values = torch.tensor([record.next_value for record in records])
        terminals = torch.tensor([record.terminal for record in records], dtype=torch.float32)
        delta = rewards + config.gamma * next_values - values
        trace_discount = config.gamma * config.gae_lambda * (1.0 - terminals)
        running = delta + trace_discount * running
        terminal_rewards = torch.tensor([record.terminal_reward for record in records])
        shaping_rewards = torch.tensor([record.shaping_reward for record in records])
        offense_rewards = torch.tensor([record.offense_reward for record in records])
        running_terminal = terminal_rewards + trace_discount * running_terminal
        running_shaping = shaping_rewards + trace_discount * running_shaping
        running_offense = offense_rewards + trace_discount * running_offense
        advantages[step] = running
        terminal_traces[step] = running_terminal
        shaping_traces[step] = running_shaping
        offense_traces[step] = running_offense

    flattened = [record for records in by_time for record in records]
    for record, advantage in zip(flattened, advantages.flatten(), strict=True):
        record.advantage = float(advantage.item())
        record.return_target = record.advantage + record.old_value
    learner_transitions = [record for record in flattened if record.is_learner]
    learner_mask = torch.tensor([record.is_learner for record in flattened], dtype=torch.bool)
    learner_terminal_traces = terminal_traces.flatten()[learner_mask]
    learner_shaping_traces = shaping_traces.flatten()[learner_mask]
    learner_offense_traces = offense_traces.flatten()[learner_mask]
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
                "rollout_candidate_afterstates": all_candidate_count_sum,
                "rollout_environment_seconds": environment_seconds,
                "rollout_policy_seconds": policy_seconds,
                "rollout_value_reward_seconds": value_reward_seconds,
                "shaping_reward_mean_abs": shaping_absolute_sum / learner_decisions,
                "shaping_reward_nonzero_rate": shaping_nonzero / learner_decisions,
                "shaping_reward_max_abs": shaping_max_absolute,
                "offense_reward_mean": offense_reward_sum / learner_decisions,
                "offense_reward_mean_abs": offense_reward_absolute_sum / learner_decisions,
                "offense_reward_nonzero_rate": offense_reward_nonzero / learner_decisions,
                "offense_reward_max_abs": offense_reward_max_absolute,
                "terminal_trace_mean_abs": float(learner_terminal_traces.abs().mean().item()),
                "terminal_trace_nonzero_rate": float(
                    (learner_terminal_traces.abs() > 1e-12).float().mean().item()
                ),
                "shaping_trace_mean_abs": float(learner_shaping_traces.abs().mean().item()),
                "shaping_trace_nonzero_rate": float(
                    (learner_shaping_traces.abs() > 1e-12).float().mean().item()
                ),
                "offense_trace_mean_abs": float(learner_offense_traces.abs().mean().item()),
                "offense_trace_nonzero_rate": float(
                    (learner_offense_traces.abs() > 1e-12).float().mean().item()
                ),
                "terminal_shaping_trace_cosine": _cosine_similarity(
                    learner_terminal_traces, learner_shaping_traces
                ),
                "terminal_offense_trace_cosine": _cosine_similarity(
                    learner_terminal_traces, learner_offense_traces
                ),
                "terminal_transition_rate": counters["learner_terminal_decisions"]
                / learner_decisions,
                "nonzero_terminal_reward_rate": (
                    counters["learner_terminal_wins"] + counters["learner_terminal_losses"]
                )
                / learner_decisions,
                "lines_per_piece": counters["learner_lines"] / learner_decisions,
                "attack_per_piece": counters["learner_attack"] / learner_decisions,
                "outgoing_attack_per_piece": counters["learner_outgoing_attack"]
                / learner_decisions,
                "cancelled_attack_per_piece": counters["learner_cancelled_attack"]
                / learner_decisions,
                "mean_max_height": counters["learner_max_height_sum"] / learner_decisions,
                "mean_holes": counters["learner_holes_sum"] / learner_decisions,
                "mean_pending_garbage": counters["learner_pending_sum"] / learner_decisions,
                "mean_ready_garbage": counters["learner_ready_sum"] / learner_decisions,
                "danger_rate": counters["learner_danger_decisions"] / learner_decisions,
                "tetris_per_100": 100.0 * counters["learner_tetrises"] / learner_decisions,
                "t_spin_mini_per_100": 100.0 * counters["learner_t_spin_mini"] / learner_decisions,
                "t_spin_full_per_100": 100.0 * counters["learner_t_spin_full"] / learner_decisions,
                "perfect_clear_per_100": 100.0
                * counters["learner_perfect_clears"]
                / learner_decisions,
            }
        )
        for index, name in enumerate(POTENTIAL_COMPONENT_NAMES):
            rollout_metrics[f"shaping_{name}_mean_abs"] = (
                float(shaping_component_absolute_sums[index].item()) / learner_decisions
            )
            rollout_metrics[f"shaping_{name}_mean"] = (
                float(shaping_component_signed_sums[index].item()) / learner_decisions
            )
    return learner_transitions, observation, seed_index, len(flattened), rollout_metrics


def _batched_actor_logits(
    observation: VersusObservation,
    actor_assignments: list[VersusActorCritic | None],
    learner: VersusActorCritic,
) -> list[Tensor]:
    if len(actor_assignments) != observation.decision_count:
        raise ValueError("actor assignment count differs from observation decisions")
    grouped: dict[int, tuple[VersusActorCritic, list[int]]] = {}
    for decision, assigned in enumerate(actor_assignments):
        actor = assigned or learner
        key = id(actor)
        if key not in grouped:
            grouped[key] = (actor, [])
        grouped[key][1].append(decision)

    logits_by_decision: list[Tensor | None] = [None] * observation.decision_count
    with torch.no_grad():
        for actor, decisions in grouped.values():
            candidate_views = [
                observation.candidate_features[
                    observation.offsets[decision] : observation.offsets[decision + 1]
                ]
                for decision in decisions
            ]
            combined = (
                candidate_views[0] if len(candidate_views) == 1 else torch.cat(candidate_views)
            )
            combined_logits = actor.actor_logits(combined)
            offset = 0
            for decision, candidates in zip(decisions, candidate_views, strict=True):
                end = offset + candidates.shape[0]
                logits_by_decision[decision] = combined_logits[offset:end]
                offset = end
    if any(logits is None for logits in logits_by_decision):
        raise RuntimeError("batched actor inference left a decision without logits")
    return [logits for logits in logits_by_decision if logits is not None]


def _segmented_log_probabilities(values: Tensor, candidate_counts: Tensor) -> Tensor:
    """Log-softmax each consecutive variable-size segment without Python loops."""
    decision_count = candidate_counts.shape[0]
    segment = torch.repeat_interleave(
        torch.arange(decision_count, device=values.device), candidate_counts
    )
    maxima = torch.full(
        (decision_count,),
        -torch.inf,
        dtype=values.dtype,
        device=values.device,
    )
    maxima.scatter_reduce_(0, segment, values, reduce="amax", include_self=True)
    exponentials = torch.exp(values - maxima[segment])
    normalizers = torch.zeros_like(maxima)
    normalizers.scatter_add_(0, segment, exponentials)
    return values - (maxima + normalizers.log())[segment]


def _ragged_policy_terms(
    logits: Tensor,
    teacher_log_probability: Tensor,
    candidate_counts: Tensor,
    actions: Tensor,
) -> tuple[Tensor, Tensor, Tensor, Tensor, Tensor]:
    """Vectorized categorical terms for concatenated variable-size decisions."""
    decision_count = candidate_counts.shape[0]
    segment = torch.repeat_interleave(
        torch.arange(decision_count, device=logits.device), candidate_counts
    )

    student_log_probability = _segmented_log_probabilities(logits, candidate_counts)
    student_probability = student_log_probability.exp()
    entropy = torch.zeros(decision_count, dtype=logits.dtype, device=logits.device).scatter_add_(
        0, segment, -(student_probability * student_log_probability)
    )
    normalizers = torch.where(
        candidate_counts > 1,
        candidate_counts.to(logits.dtype).log(),
        torch.ones(decision_count, dtype=logits.dtype, device=logits.device),
    )
    normalized_entropy = torch.where(
        candidate_counts > 1,
        entropy / normalizers,
        torch.zeros_like(entropy),
    )
    starts = torch.cumsum(candidate_counts, dim=0) - candidate_counts
    selected_log_probability = student_log_probability[starts + actions]

    teacher_probability = teacher_log_probability.exp()
    kickstart = torch.zeros(decision_count, dtype=logits.dtype, device=logits.device).scatter_add_(
        0,
        segment,
        teacher_probability * (teacher_log_probability - student_log_probability),
    )
    return (
        selected_log_probability,
        entropy,
        normalized_entropy,
        kickstart,
        student_log_probability,
    )


def _ragged_kl_from_log_probabilities(
    student_log_probability: Tensor,
    target_log_probability: Tensor,
    candidate_counts: Tensor,
) -> Tensor:
    decision_count = candidate_counts.shape[0]
    segment = torch.repeat_interleave(
        torch.arange(decision_count, device=student_log_probability.device), candidate_counts
    )
    target_probability = target_log_probability.exp()
    return torch.zeros(
        decision_count,
        dtype=student_log_probability.dtype,
        device=student_log_probability.device,
    ).scatter_add_(
        0,
        segment,
        target_probability * (target_log_probability - student_log_probability),
    )


def _ragged_target_kl(
    student_logits: Tensor,
    target_logits: Tensor,
    candidate_counts: Tensor,
) -> Tensor:
    """KL(target || student) for consecutive variable-size decisions."""
    student_log_probability = _segmented_log_probabilities(student_logits, candidate_counts)
    target_log_probability = _segmented_log_probabilities(target_logits, candidate_counts)
    return _ragged_kl_from_log_probabilities(
        student_log_probability, target_log_probability, candidate_counts
    )


def _precompute_teacher_log_probabilities(
    teacher: VersusActorCritic,
    transitions: list[DecisionTransition],
    chunk_decisions: int,
) -> list[Tensor]:
    cached: list[Tensor] = []
    with torch.no_grad():
        for start in range(0, len(transitions), chunk_decisions):
            records = transitions[start : start + chunk_decisions]
            candidates = torch.cat([record.candidates for record in records], dim=0)
            counts = torch.tensor(
                [record.candidates.shape[0] for record in records], dtype=torch.long
            )
            log_probability = _segmented_log_probabilities(teacher.actor_logits(candidates), counts)
            cached.extend(torch.split(log_probability, counts.tolist()))
    return cached


def _segmented_unit_range(values: Tensor, candidate_counts: Tensor) -> Tensor:
    decision_count = candidate_counts.shape[0]
    segment = torch.repeat_interleave(
        torch.arange(decision_count, device=values.device), candidate_counts
    )
    maxima = torch.full((decision_count,), -torch.inf, dtype=values.dtype, device=values.device)
    minima = torch.full((decision_count,), torch.inf, dtype=values.dtype, device=values.device)
    maxima.scatter_reduce_(0, segment, values, reduce="amax", include_self=True)
    minima.scatter_reduce_(0, segment, values, reduce="amin", include_self=True)
    ranges = (maxima - minima).clamp_min(1e-6)
    return (values - minima[segment]) / ranges[segment]


def _segmented_argmax(values: Tensor, candidate_counts: Tensor) -> Tensor:
    decision_count = candidate_counts.shape[0]
    segment = torch.repeat_interleave(
        torch.arange(decision_count, device=values.device), candidate_counts
    )
    maxima = torch.full((decision_count,), -torch.inf, dtype=values.dtype, device=values.device)
    maxima.scatter_reduce_(0, segment, values, reduce="amax", include_self=True)
    starts = torch.cumsum(candidate_counts, dim=0) - candidate_counts
    local_indices = torch.arange(values.shape[0], device=values.device) - starts[segment]
    sentinel = candidate_counts.max()
    candidates = torch.where(values == maxima[segment], local_indices, sentinel)
    actions = torch.full_like(candidate_counts, sentinel)
    actions.scatter_reduce_(0, segment, candidates, reduce="amin", include_self=True)
    return actions


def _precompute_tactical_target_actions(
    teacher_log_probabilities: list[Tensor],
    transitions: list[DecisionTransition],
    config: SelfPlayConfig,
    reward_config: PotentialConfig,
    chunk_decisions: int,
) -> tuple[list[int], float]:
    cached: list[int] = []
    changed = 0
    with torch.no_grad():
        for start in range(0, len(transitions), chunk_decisions):
            records = transitions[start : start + chunk_decisions]
            counts = torch.tensor(
                [record.candidates.shape[0] for record in records], dtype=torch.long
            )
            teacher_log_probability = torch.cat(
                teacher_log_probabilities[start : start + chunk_decisions]
            )
            diagnostics = torch.cat([record.candidate_diagnostics for record in records], dim=0)
            teacher_unit_range = _segmented_unit_range(teacher_log_probability, counts)
            target_scores = teacher_unit_range + (
                tactical_candidate_scores(diagnostics, reward_config)
                / config.tactical_curriculum_temperature
            )
            target_actions = _segmented_argmax(target_scores, counts)
            teacher_actions = _segmented_argmax(teacher_log_probability, counts)
            changed += int((target_actions != teacher_actions).sum().item())
            cached.extend(target_actions.tolist())
    return cached, changed / len(transitions)


def _ppo_update(
    model: VersusActorCritic,
    teacher: VersusActorCritic,
    optimizer: torch.optim.Optimizer,
    transitions: list[DecisionTransition],
    config: SelfPlayConfig,
    entropy_coefficient: float,
    kickstart_coefficient: float,
    tactical_coefficient: float,
    reward_config: PotentialConfig,
) -> dict[str, float]:
    advantages = torch.tensor([record.advantage for record in transitions])
    advantages = (advantages - advantages.mean()) / advantages.std().clamp_min(1e-6)
    teacher_log_probabilities = _precompute_teacher_log_probabilities(
        teacher, transitions, config.minibatch_decisions * 4
    )
    tactical_target_actions, tactical_target_change_rate = (
        _precompute_tactical_target_actions(
            teacher_log_probabilities,
            transitions,
            config,
            reward_config,
            config.minibatch_decisions * 4,
        )
        if tactical_coefficient > 0.0
        else ([], 0.0)
    )
    losses: list[float] = []
    policy_losses: list[float] = []
    value_losses: list[float] = []
    entropies: list[float] = []
    normalized_entropies: list[float] = []
    approximate_kls: list[float] = []
    clip_fractions: list[float] = []
    gradient_norms: list[float] = []
    explained_variances: list[float] = []
    kickstart_losses: list[float] = []
    tactical_losses: list[float] = []
    extra_value_losses: list[float] = []
    extra_value_gradient_norms: list[float] = []
    for _ in range(config.ppo_epochs):
        order = torch.randperm(len(transitions)).tolist()
        for start in range(0, len(order), config.minibatch_decisions):
            indices = order[start : start + config.minibatch_decisions]
            records = [transitions[index] for index in indices]
            candidate_tensor = torch.cat([record.candidates for record in records], dim=0)
            logits = model.actor_logits(candidate_tensor)
            teacher_log_probability = torch.cat(
                [teacher_log_probabilities[index] for index in indices]
            )
            candidate_counts = torch.tensor(
                [record.candidates.shape[0] for record in records], dtype=torch.long
            )
            actions = torch.tensor([record.action for record in records], dtype=torch.long)
            (
                new_log_probability,
                entropy_values,
                normalized_entropy_values,
                kickstart_values,
                student_log_probability,
            ) = _ragged_policy_terms(logits, teacher_log_probability, candidate_counts, actions)
            entropy = entropy_values.mean()
            normalized_entropy = normalized_entropy_values.mean()
            kickstart_loss = kickstart_values.mean()
            if tactical_coefficient > 0.0:
                tactical_actions = torch.tensor(
                    [tactical_target_actions[index] for index in indices], dtype=torch.long
                )
                starts = torch.cumsum(candidate_counts, dim=0) - candidate_counts
                tactical_loss = -student_log_probability[starts + tactical_actions].mean()
            else:
                tactical_loss = torch.zeros((), dtype=logits.dtype, device=logits.device)
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
                + kickstart_coefficient * kickstart_loss
                + tactical_coefficient * tactical_loss
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
            kickstart_losses.append(float(kickstart_loss.item()))
            tactical_losses.append(float(tactical_loss.item()))

    if config.value_extra_epochs > 0:
        value_parameters = list(model.value_core.parameters())
        for _ in range(config.value_extra_epochs):
            order = torch.randperm(len(transitions)).tolist()
            for start in range(0, len(order), config.minibatch_decisions):
                indices = order[start : start + config.minibatch_decisions]
                states = torch.stack([transitions[index].state for index in indices])
                targets = torch.tensor([transitions[index].return_target for index in indices])
                value_loss = torch.nn.functional.mse_loss(model.value(states), targets)
                optimizer.zero_grad(set_to_none=True)
                (config.value_coefficient * value_loss).backward()
                gradient_norm = torch.nn.utils.clip_grad_norm_(
                    value_parameters, config.max_grad_norm
                )
                optimizer.step()
                extra_value_losses.append(float(value_loss.item()))
                extra_value_gradient_norms.append(float(gradient_norm.item()))

    all_states = torch.stack([record.state for record in transitions])
    all_targets = torch.tensor([record.return_target for record in transitions])
    with torch.no_grad():
        postfit_values = model.value(all_states)
    target_variance = all_targets.var(unbiased=False)
    postfit_explained_variance = 0.0
    if float(target_variance.item()) > 1e-12:
        residual_variance = (all_targets - postfit_values).var(unbiased=False)
        postfit_explained_variance = float((1.0 - residual_variance / target_variance).item())

    return {
        "loss": _mean(losses),
        "policy_loss": _mean(policy_losses),
        "value_loss": _mean(value_losses),
        "ppo_entropy": _mean(entropies),
        "ppo_normalized_entropy": _mean(normalized_entropies),
        "entropy_loss_contribution": -entropy_coefficient
        * _mean(normalized_entropies if config.normalize_entropy else entropies),
        "kickstart_kl": _mean(kickstart_losses),
        "kickstart_loss_contribution": kickstart_coefficient * _mean(kickstart_losses),
        "tactical_curriculum_cross_entropy": _mean(tactical_losses),
        "tactical_curriculum_loss_contribution": tactical_coefficient * _mean(tactical_losses),
        "tactical_target_change_rate": tactical_target_change_rate,
        "approximate_kl": _mean(approximate_kls),
        "clip_fraction": _mean(clip_fractions),
        "gradient_norm": _mean(gradient_norms),
        "explained_variance": _mean(explained_variances) if explained_variances else 0.0,
        "value_postfit_explained_variance": postfit_explained_variance,
        "value_extra_loss": _mean(extra_value_losses),
        "value_extra_gradient_norm": _mean(extra_value_gradient_norms),
        "mean_reward": _mean([record.reward for record in transitions]),
    }


def _load_opponent_pool(
    output_dir: Path,
    config: SelfPlayConfig,
    allow_observed: bool,
    pool_state: OpponentPoolState | None = None,
) -> list[OpponentPoolEntry]:
    if pool_state is None:
        snapshots = sorted((output_dir / "snapshots").glob("update-*-model.pt"))
        paths = [Path(path) for path in config.anchor_checkpoints or []]
        paths.extend(_stratified_paths(snapshots, config.opponent_pool_limit))
    else:
        paths = [Path(path) for path in pool_state.checkpoints()]
    unique_paths = list(dict.fromkeys(paths))
    entries = []
    for path in unique_paths:
        if not path.is_file():
            raise FileNotFoundError(f"configured opponent checkpoint not found: {path}")
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
    league_stats: dict[str, dict[str, int]] | None = None,
    *,
    pool_state: OpponentPoolState | None = None,
    update: int = 0,
) -> MatchAssignment:
    self_play_matches = round(config.parallel_matches * config.self_play_fraction)
    historical_matches = round(config.parallel_matches * config.historical_fraction)
    if match_index < self_play_matches:
        return MatchAssignment(kind="self_play", learner_side=None)
    learner_side = seed % 2
    if match_index < self_play_matches + historical_matches and historical:
        if pool_state is None:
            opponent = _sample_pfsp_opponent(config, seed, historical, league_stats or {})
            opponent_checkpoint = opponent.checkpoint
            sampling_mode = "legacy"
        else:
            opponent_checkpoint, sampling_mode = pool_state.sample(
                seed,
                update,
                half_life_updates=config.opponent_score_half_life_updates,
                balanced_fraction=config.historical_balanced_fraction,
                hard_fraction=config.historical_hard_fraction,
                uniform_fraction=config.historical_uniform_fraction,
                exponent=config.pfsp_exponent,
                min_weight=config.pfsp_min_weight,
            )
        return MatchAssignment(
            kind="historical",
            learner_side=learner_side,
            opponent_checkpoint=opponent_checkpoint,
            sampling_mode=sampling_mode,
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


def _kickstart_coefficient(config: SelfPlayConfig, update: int) -> float:
    fraction = min(max(update, 0) / config.kickstart_decay_updates, 1.0)
    return config.kickstart_coefficient + fraction * (
        config.kickstart_coefficient_final - config.kickstart_coefficient
    )


def _tactical_curriculum_coefficient(config: SelfPlayConfig, update: int) -> float:
    fraction = min(max(update, 0) / config.tactical_curriculum_decay_updates, 1.0)
    return config.tactical_curriculum_coefficient + fraction * (
        config.tactical_curriculum_coefficient_final - config.tactical_curriculum_coefficient
    )


def _offense_reward_coefficient(config: SelfPlayConfig, update: int) -> float:
    if config.schema_version != "versus-selfplay-ppo-v6":
        return 0.0
    if update <= config.offense_reward_hold_updates:
        return config.offense_reward_coefficient
    fraction = min(
        (update - config.offense_reward_hold_updates)
        / (config.offense_reward_decay_updates - config.offense_reward_hold_updates),
        1.0,
    )
    return config.offense_reward_coefficient + fraction * (
        config.offense_reward_coefficient_final - config.offense_reward_coefficient
    )


def _net_offense_rewards(outgoing: Tensor, coefficient: float, attack_scale: float) -> Tensor:
    if outgoing.ndim != 1 or outgoing.numel() % 2 != 0:
        raise ValueError("outgoing attack must contain player pairs")
    if coefficient < 0.0 or attack_scale <= 0.0:
        raise ValueError("invalid offense reward parameters")
    paired = outgoing.reshape(-1, 2)
    relative = torch.clamp((paired[:, 0] - paired[:, 1]) / attack_scale, -1.0, 1.0)
    rewards = coefficient * relative
    return torch.stack((rewards, -rewards), dim=1).reshape(-1)


def _build_optimizer(model: VersusActorCritic, config: SelfPlayConfig) -> torch.optim.Optimizer:
    if model.config.architecture_version == 1:
        return torch.optim.Adam(model.parameters(), lr=config.learning_rate)
    solo_parameters = list(model.solo_model.parameters())
    if config.schema_version in {"versus-selfplay-ppo-v5", "versus-selfplay-ppo-v6"}:
        value_parameters = list(model.value_core.parameters())
        actor_parameters = [
            parameter
            for name, parameter in model.named_parameters()
            if not name.startswith("solo_model.") and not name.startswith("value_core.")
        ]
        return torch.optim.Adam(
            [
                {"params": actor_parameters, "lr": config.learning_rate},
                {
                    "params": value_parameters,
                    "lr": config.learning_rate * config.value_learning_rate_multiplier,
                },
                {
                    "params": solo_parameters,
                    "lr": config.learning_rate * config.solo_learning_rate_multiplier,
                },
            ]
        )
    adaptation_parameters = [
        parameter
        for name, parameter in model.named_parameters()
        if not name.startswith("solo_model.")
    ]
    return torch.optim.Adam(
        [
            {"params": adaptation_parameters, "lr": config.learning_rate},
            {
                "params": solo_parameters,
                "lr": config.learning_rate * config.solo_learning_rate_multiplier,
            },
        ]
    )


def _stratified_paths(paths: list[Path], limit: int) -> list[Path]:
    if limit <= 0 or not paths:
        return []
    if len(paths) <= limit:
        return paths
    if limit == 1:
        return [paths[-1]]
    indices = [round(index * (len(paths) - 1) / (limit - 1)) for index in range(limit)]
    return [paths[index] for index in indices]


def _sample_pfsp_opponent(
    config: SelfPlayConfig,
    seed: int,
    historical: list[OpponentPoolEntry],
    league_stats: dict[str, dict[str, int]],
) -> OpponentPoolEntry:
    weights = []
    for entry in historical:
        stats = league_stats.get(entry.checkpoint, {})
        games = int(stats.get("games", 0))
        score = (int(stats.get("wins", 0)) + 0.5 * int(stats.get("draws", 0)) + 1.0) / (games + 2.0)
        weights.append(max(config.pfsp_min_weight, (1.0 - score) ** config.pfsp_exponent))
    chooser = random.Random(seed ^ 0x5EED5EED)
    return chooser.choices(historical, weights=weights, k=1)[0]


def _opponent_pool_metrics(
    pool_state: OpponentPoolState, config: SelfPlayConfig, update: int
) -> dict[str, float | int]:
    return pool_state.metrics(
        update,
        half_life_updates=config.opponent_score_half_life_updates,
        balanced_fraction=config.historical_balanced_fraction,
        hard_fraction=config.historical_hard_fraction,
        uniform_fraction=config.historical_uniform_fraction,
        exponent=config.pfsp_exponent,
        min_weight=config.pfsp_min_weight,
    )


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
    league_stats: dict[str, dict[str, int]],
    pool_state: OpponentPoolState | None = None,
) -> None:
    payload = {
        "checkpoint_schema": (
            CHECKPOINT_SCHEMA_V6
            if config.schema_version == "versus-selfplay-ppo-v6"
            else CHECKPOINT_SCHEMA_V5
            if config.schema_version == "versus-selfplay-ppo-v5"
            else CHECKPOINT_SCHEMA
        ),
        "config": _config_payload(config),
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
        "league_stats": league_stats,
        "opponent_pool_state": pool_state.to_payload() if pool_state is not None else None,
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
            "checkpoint_schema": (
                "versus-actor-critic-v1"
                if model.config.architecture_version == 1
                else "versus-actor-critic-v2"
            ),
            "mechanics_status": MECHANICS_STATUS,
            "config": _config_payload(config),
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
    expected_schema = (
        CHECKPOINT_SCHEMA_V6
        if config.schema_version == "versus-selfplay-ppo-v6"
        else CHECKPOINT_SCHEMA_V5
        if config.schema_version == "versus-selfplay-ppo-v5"
        else CHECKPOINT_SCHEMA
    )
    if payload.get("checkpoint_schema") != expected_schema:
        raise ValueError("incompatible self-play checkpoint schema")
    if (
        payload.get("config") != _config_payload(config)
        or payload.get("config_sha256") != config_hash
    ):
        raise ValueError("self-play config changed; refusing semantic resume")
    if payload.get("bootstrap_sha256") != bootstrap_hash:
        raise ValueError("solo bootstrap changed; refusing resume")
    if "environment_state" not in payload:
        raise ValueError("progress checkpoint lacks exact environment state")
    if "match_assignments" not in payload:
        raise ValueError("progress checkpoint lacks persistent opponent assignments")
    if config.schema_version in {
        "versus-selfplay-ppo-v5",
        "versus-selfplay-ppo-v6",
    } and not isinstance(payload.get("opponent_pool_state"), dict):
        raise ValueError("progress checkpoint lacks stable opponent-pool state")


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


def _cosine_similarity(one: Tensor, two: Tensor) -> float:
    denominator = float((torch.linalg.vector_norm(one) * torch.linalg.vector_norm(two)).item())
    if denominator <= 1e-12:
        return 0.0
    return float(torch.dot(one, two).item()) / denominator


def _config_payload(config: SelfPlayConfig) -> dict[str, object]:
    payload = asdict(config)
    if config.schema_version not in {
        "versus-selfplay-ppo-v4",
        "versus-selfplay-ppo-v5",
        "versus-selfplay-ppo-v6",
    }:
        for name in (
            "tactical_potential_fraction",
            "tactical_curriculum_coefficient",
            "tactical_curriculum_coefficient_final",
            "tactical_curriculum_decay_updates",
            "tactical_curriculum_temperature",
        ):
            payload.pop(name)
    if config.schema_version not in {"versus-selfplay-ppo-v5", "versus-selfplay-ppo-v6"}:
        for name in (
            "pool_promotion_interval_updates",
            "opponent_recent_slots",
            "opponent_score_half_life_updates",
            "opponent_result_history_limit",
            "historical_balanced_fraction",
            "historical_hard_fraction",
            "historical_uniform_fraction",
            "value_learning_rate_multiplier",
            "value_extra_epochs",
        ):
            payload.pop(name)
    if config.schema_version != "versus-selfplay-ppo-v6":
        for name in (
            "offense_reward_coefficient",
            "offense_reward_coefficient_final",
            "offense_reward_hold_updates",
            "offense_reward_decay_updates",
            "offense_reward_attack_scale",
        ):
            payload.pop(name)
    return payload


def _as_random_state(value: object) -> tuple:
    if not isinstance(value, tuple):
        raise ValueError("invalid Python RNG state")
    return value


if __name__ == "__main__":
    main()
