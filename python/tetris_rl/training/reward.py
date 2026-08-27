from dataclasses import dataclass

import torch
from torch import Tensor

STATE_FEATURE_COUNT = 12


@dataclass(frozen=True)
class PotentialConfig:
    gamma: float = 0.997
    shaping_scale: float = 0.10
    weights: tuple[float, ...] = (0.35, 0.20, 0.25, 0.10, 0.05, 0.05)
    bounds: tuple[float, ...] = (20.0, 16.0, 20.0, 8.0, 4.0, 8.0)

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


def state_potential(state_features: Tensor, config: PotentialConfig) -> Tensor:
    """Bounded player-relative Phi(s); swapping players negates it exactly."""
    config.validate()
    if state_features.ndim != 2 or state_features.shape[1] != STATE_FEATURE_COUNT:
        raise ValueError(
            f"expected [states, {STATE_FEATURE_COUNT}], got {tuple(state_features.shape)}"
        )
    own = state_features[:, 0::2]
    opponent = state_features[:, 1::2]
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
    return normalized @ weights


def transition_reward(
    current_state: Tensor,
    next_state: Tensor,
    terminal_outcome: Tensor,
    terminal: Tensor,
    config: PotentialConfig,
) -> Tensor:
    """z + lambda * (gamma*Phi(s') - Phi(s)), with Phi(terminal)=0."""
    if terminal_outcome.ndim != 1 or terminal.ndim != 1:
        raise ValueError("terminal outcome and mask must be vectors")
    if terminal_outcome.shape != terminal.shape or terminal.shape[0] != current_state.shape[0]:
        raise ValueError("reward batch dimensions differ")
    current = state_potential(current_state, config)
    following = state_potential(next_state, config)
    following = torch.where(terminal, torch.zeros_like(following), following)
    shaping = config.gamma * following - current
    return terminal_outcome + config.shaping_scale * shaping
