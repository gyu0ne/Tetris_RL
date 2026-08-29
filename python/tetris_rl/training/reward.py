from dataclasses import dataclass

import torch
from torch import Tensor

STATE_FEATURE_COUNT = 12
CANDIDATE_DIAGNOSTIC_COUNT = 5
STRUCTURAL_COMPONENT_NAMES = (
    "stack",
    "holes",
    "pending",
    "ready",
    "combo",
    "back_to_back",
)
TACTICAL_COMPONENT_NAMES = (
    "attack_readiness",
    "defense_readiness",
    "full_t_spin_readiness",
)
POTENTIAL_COMPONENT_NAMES = STRUCTURAL_COMPONENT_NAMES + TACTICAL_COMPONENT_NAMES


@dataclass(frozen=True)
class PotentialConfig:
    gamma: float = 0.997
    shaping_scale: float = 0.10
    weights: tuple[float, ...] = (0.25, 0.25, 0.15, 0.25, 0.05, 0.05)
    bounds: tuple[float, ...] = (20.0, 16.0, 20.0, 8.0, 4.0, 8.0)
    tactical_fraction: float = 0.0
    tactical_weights: tuple[float, ...] = (0.4, 0.4, 0.2)
    tactical_bounds: tuple[float, ...] = (8.0, 8.0, 8.0)

    def validate(self) -> None:
        if not 0.0 < self.gamma <= 1.0:
            raise ValueError("gamma must be in (0, 1]")
        if not 0.0 <= self.shaping_scale <= 1.0:
            raise ValueError("shaping_scale must be in [0, 1]")
        if len(self.weights) != 6 or len(self.bounds) != 6:
            raise ValueError("potential requires six paired features")
        if any(bound <= 0.0 for bound in self.bounds):
            raise ValueError("potential bounds must be positive")
        if abs(sum(self.weights) - 1.0) > 1e-6 or any(weight < 0.0 for weight in self.weights):
            raise ValueError("potential weights must be nonnegative and sum to one")
        if not 0.0 <= self.tactical_fraction <= 1.0:
            raise ValueError("tactical fraction must be in [0, 1]")
        if len(self.tactical_weights) != 3 or len(self.tactical_bounds) != 3:
            raise ValueError("tactical potential requires three features")
        if any(bound <= 0.0 for bound in self.tactical_bounds):
            raise ValueError("tactical bounds must be positive")
        if abs(sum(self.tactical_weights) - 1.0) > 1e-6 or any(
            weight < 0.0 for weight in self.tactical_weights
        ):
            raise ValueError("tactical weights must be nonnegative and sum to one")


def state_potential(state_features: Tensor, config: PotentialConfig) -> Tensor:
    """Bounded player-relative Phi(s); swapping players negates it exactly."""
    return state_potential_components(state_features, config).sum(dim=1)


def state_potential_components(state_features: Tensor, config: PotentialConfig) -> Tensor:
    """Weighted, bounded components whose row sum is Phi(s)."""
    config.validate()
    if state_features.ndim != 2 or state_features.shape[1] < STATE_FEATURE_COUNT:
        raise ValueError(
            f"expected [states, at least {STATE_FEATURE_COUNT}], got {tuple(state_features.shape)}"
        )
    paired = state_features[:, :STATE_FEATURE_COUNT]
    own = paired[:, 0::2]
    opponent = paired[:, 1::2]
    # Stack pressure is good when it is on the opponent. Chain state is good
    # when it belongs to the acting player.
    raw = torch.stack(
        (
            opponent[:, 0] - own[:, 0],
            opponent[:, 1] - own[:, 1],
            opponent[:, 2] - own[:, 2],
            opponent[:, 3] - own[:, 3],
            own[:, 4] - opponent[:, 4],
            own[:, 5] - opponent[:, 5],
        ),
        dim=1,
    )
    bounds = torch.tensor(config.bounds, dtype=state_features.dtype, device=state_features.device)
    weights = torch.tensor(config.weights, dtype=state_features.dtype, device=state_features.device)
    normalized = torch.clamp(raw / bounds, min=-1.0, max=1.0)
    return normalized * weights


def tactical_candidate_scores(candidate_diagnostics: Tensor, config: PotentialConfig) -> Tensor:
    """Bounded per-candidate attack, cancellation and full-spin curriculum score."""
    config.validate()
    if (
        candidate_diagnostics.ndim != 2
        or candidate_diagnostics.shape[1] < CANDIDATE_DIAGNOSTIC_COUNT
    ):
        raise ValueError(
            "expected [candidates, at least "
            f"{CANDIDATE_DIAGNOSTIC_COUNT}], got {tuple(candidate_diagnostics.shape)}"
        )
    t_spin = candidate_diagnostics[:, 1]
    attack = candidate_diagnostics[:, 3]
    outgoing = candidate_diagnostics[:, 4]
    raw = torch.stack(
        (
            outgoing,
            torch.clamp(attack - outgoing, min=0.0),
            torch.where(t_spin == 2.0, attack, torch.zeros_like(attack)),
        ),
        dim=1,
    )
    bounds = torch.tensor(
        config.tactical_bounds,
        dtype=candidate_diagnostics.dtype,
        device=candidate_diagnostics.device,
    )
    weights = torch.tensor(
        config.tactical_weights,
        dtype=candidate_diagnostics.dtype,
        device=candidate_diagnostics.device,
    )
    return (torch.clamp(raw / bounds, min=0.0, max=1.0) * weights).sum(dim=1)


def tactical_potential_components(
    candidate_diagnostics: Tensor,
    offsets: tuple[int, ...],
    config: PotentialConfig,
) -> Tensor:
    """Player-relative readiness from each side's best currently legal candidate."""
    config.validate()
    decision_count = len(offsets) - 1
    if decision_count < 0 or decision_count % 2 != 0:
        raise ValueError("tactical decisions must contain player pairs")
    if offsets[0] != 0 or offsets[-1] != candidate_diagnostics.shape[0]:
        raise ValueError("candidate offsets do not cover diagnostics")
    if any(start > end for start, end in zip(offsets, offsets[1:], strict=False)):
        raise ValueError("candidate offsets must be nondecreasing")
    if (
        candidate_diagnostics.ndim != 2
        or candidate_diagnostics.shape[1] < CANDIDATE_DIAGNOSTIC_COUNT
    ):
        raise ValueError(
            "expected [candidates, at least "
            f"{CANDIDATE_DIAGNOSTIC_COUNT}], got {tuple(candidate_diagnostics.shape)}"
        )

    local = torch.zeros(
        (decision_count, 3),
        dtype=candidate_diagnostics.dtype,
        device=candidate_diagnostics.device,
    )
    bounds = torch.tensor(
        config.tactical_bounds,
        dtype=candidate_diagnostics.dtype,
        device=candidate_diagnostics.device,
    )
    weights = torch.tensor(
        config.tactical_weights,
        dtype=candidate_diagnostics.dtype,
        device=candidate_diagnostics.device,
    )
    for decision, (start, end) in enumerate(zip(offsets, offsets[1:], strict=False)):
        if start == end:
            continue
        diagnostics = candidate_diagnostics[start:end]
        attack = diagnostics[:, 3]
        outgoing = diagnostics[:, 4]
        local[decision] = torch.stack(
            (
                outgoing.max(),
                torch.clamp(attack - outgoing, min=0.0).max(),
                torch.where(
                    diagnostics[:, 1] == 2.0,
                    attack,
                    torch.zeros_like(attack),
                ).max(),
            )
        )
    local = torch.clamp(local / bounds, min=0.0, max=1.0) * weights
    paired = local.reshape(-1, 2, 3)
    relative = paired[:, 0] - paired[:, 1]
    return torch.stack((relative, -relative), dim=1).reshape(decision_count, 3)


def transition_reward(
    current_state: Tensor,
    next_state: Tensor,
    terminal_outcome: Tensor,
    terminal: Tensor,
    config: PotentialConfig,
    *,
    current_tactical: Tensor | None = None,
    next_tactical: Tensor | None = None,
) -> Tensor:
    """z + lambda * (gamma*Phi(s') - Phi(s)), with Phi(terminal)=0."""
    reward, _ = transition_reward_details(
        current_state,
        next_state,
        terminal_outcome,
        terminal,
        config,
        current_tactical=current_tactical,
        next_tactical=next_tactical,
    )
    return reward


def transition_reward_details(
    current_state: Tensor,
    next_state: Tensor,
    terminal_outcome: Tensor,
    terminal: Tensor,
    config: PotentialConfig,
    *,
    current_tactical: Tensor | None = None,
    next_tactical: Tensor | None = None,
) -> tuple[Tensor, Tensor]:
    """Returns total reward and scaled structural/tactical shaping components."""
    if terminal_outcome.ndim != 1 or terminal.ndim != 1:
        raise ValueError("terminal outcome and mask must be vectors")
    if terminal_outcome.shape != terminal.shape or terminal.shape[0] != current_state.shape[0]:
        raise ValueError("reward batch dimensions differ")
    structural_fraction = 1.0 - config.tactical_fraction
    current = structural_fraction * state_potential_components(current_state, config)
    following = structural_fraction * state_potential_components(next_state, config)
    if config.tactical_fraction > 0.0:
        expected = (current_state.shape[0], len(TACTICAL_COMPONENT_NAMES))
        if current_tactical is None or next_tactical is None:
            raise ValueError(
                "tactical potential tensors are required when its fraction is positive"
            )
        if current_tactical.shape != expected or next_tactical.shape != expected:
            raise ValueError(f"expected tactical tensors with shape {expected}")
        current = torch.cat((current, config.tactical_fraction * current_tactical), dim=1)
        following = torch.cat((following, config.tactical_fraction * next_tactical), dim=1)
    else:
        current_zeros = torch.zeros(
            (current_state.shape[0], len(TACTICAL_COMPONENT_NAMES)),
            dtype=current.dtype,
            device=current.device,
        )
        following_zeros = torch.zeros(
            (next_state.shape[0], len(TACTICAL_COMPONENT_NAMES)),
            dtype=following.dtype,
            device=following.device,
        )
        current = torch.cat((current, current_zeros), dim=1)
        following = torch.cat((following, following_zeros), dim=1)
    following = torch.where(terminal[:, None], torch.zeros_like(following), following)
    shaping = config.shaping_scale * (config.gamma * following - current)
    return terminal_outcome + shaping.sum(dim=1), shaping
