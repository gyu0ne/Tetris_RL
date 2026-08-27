from copy import deepcopy
from dataclasses import dataclass

import torch
from torch import Tensor, nn

from tetris_rl.models.checkpoint import LoadedScorer

CANDIDATE_FEATURE_COUNT = 20
STATE_FEATURE_COUNT = 12


@dataclass(frozen=True)
class VersusModelConfig:
    context_hidden: int = 32
    value_hidden: int = 32


class VersusActorCritic(nn.Module):
    def __init__(self, solo: LoadedScorer, config: VersusModelConfig | None = None) -> None:
        super().__init__()
        self.config = config or VersusModelConfig()
        self.solo_model = deepcopy(solo.model)
        self.register_buffer("solo_mean", solo.feature_mean.detach().clone())
        self.register_buffer("solo_std", solo.feature_std.detach().clone())
        self.register_buffer(
            "context_scale",
            torch.tensor((8, 8, 20, 8, 4, 8, 20, 8, 20, 16), dtype=torch.float32),
        )
        self.register_buffer(
            "state_scale", torch.tensor((20, 16, 20, 8, 4, 8), dtype=torch.float32)
        )
        self.context_model = nn.Sequential(
            nn.Linear(10, self.config.context_hidden),
            nn.Tanh(),
            nn.Linear(self.config.context_hidden, 1),
        )
        self.value_core = nn.Sequential(
            nn.Linear(6, self.config.value_hidden),
            nn.Tanh(),
            nn.Linear(self.config.value_hidden, self.config.value_hidden),
            nn.Tanh(),
            nn.Linear(self.config.value_hidden, 1),
        )
        nn.init.zeros_(self.context_model[-1].weight)
        nn.init.zeros_(self.context_model[-1].bias)

    def actor_logits(self, candidate_features: Tensor) -> Tensor:
        if candidate_features.ndim != 2 or candidate_features.shape[1] != CANDIDATE_FEATURE_COUNT:
            raise ValueError(
                f"expected [candidates, {CANDIDATE_FEATURE_COUNT}], "
                f"got {tuple(candidate_features.shape)}"
            )
        solo = (candidate_features[:, :10] - self.solo_mean) / self.solo_std
        context = candidate_features[:, 10:] / self.context_scale
        return self.solo_model(solo) + self.context_model(context).squeeze(-1)

    def value(self, state_features: Tensor) -> Tensor:
        if state_features.ndim != 2 or state_features.shape[1] != STATE_FEATURE_COUNT:
            raise ValueError(
                f"expected [states, {STATE_FEATURE_COUNT}], got {tuple(state_features.shape)}"
            )
        own = state_features[:, 0::2]
        opponent = state_features[:, 1::2]
        differences = (opponent - own) / self.state_scale
        # Odd construction guarantees V(swap(s)) = -V(s).
        return 0.5 * (
            self.value_core(differences).squeeze(-1) - self.value_core(-differences).squeeze(-1)
        )

    def parameter_count(self) -> int:
        return sum(parameter.numel() for parameter in self.parameters())
