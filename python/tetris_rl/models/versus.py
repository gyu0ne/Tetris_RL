from copy import deepcopy
from dataclasses import dataclass

import torch
from torch import Tensor, nn

from tetris_rl.models.checkpoint import LoadedScorer

LEGACY_CANDIDATE_FEATURE_COUNT = 20
CANDIDATE_FEATURE_COUNT = 76
LEGACY_STATE_FEATURE_COUNT = 12
STATE_FEATURE_COUNT = 122


@dataclass(frozen=True)
class VersusModelConfig:
    context_hidden: int = 32
    value_hidden: int = 32
    architecture_version: int = 2
    actor_hidden: int = 64
    actor_bottleneck: int = 32


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
        if self.config.architecture_version == 1:
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
        elif self.config.architecture_version == 2:
            self.register_buffer(
                "actor_extra_scale",
                torch.tensor(
                    (8, 8, 20, 8, 4, 8, 20, 8, 20, 16) + (20,) * 10 + (16,) * 10 + (1,) * 36,
                    dtype=torch.float32,
                ),
            )
            self.register_buffer(
                "player_state_scale",
                torch.tensor(
                    (20, 16, 20, 8, 4, 8) + (20,) * 10 + (16,) * 10 + (1,) * 35,
                    dtype=torch.float32,
                ),
            )
            self.residual_model = nn.Sequential(
                nn.Linear(CANDIDATE_FEATURE_COUNT, self.config.actor_hidden),
                nn.Tanh(),
                nn.Linear(self.config.actor_hidden, self.config.actor_bottleneck),
                nn.Tanh(),
                nn.Linear(self.config.actor_bottleneck, 1),
            )
            self.value_core = nn.Sequential(
                nn.Linear(STATE_FEATURE_COUNT, self.config.value_hidden),
                nn.Tanh(),
                nn.Linear(self.config.value_hidden, self.config.value_hidden),
                nn.Tanh(),
                nn.Linear(self.config.value_hidden, 1),
            )
            nn.init.zeros_(self.residual_model[-1].weight)
            nn.init.zeros_(self.residual_model[-1].bias)
        else:
            raise ValueError(f"unsupported versus architecture {self.config.architecture_version}")

    def actor_logits(self, candidate_features: Tensor) -> Tensor:
        required = (
            LEGACY_CANDIDATE_FEATURE_COUNT
            if self.config.architecture_version == 1
            else CANDIDATE_FEATURE_COUNT
        )
        if candidate_features.ndim != 2 or candidate_features.shape[1] < required:
            raise ValueError(
                f"expected [candidates, at least {required}], got {tuple(candidate_features.shape)}"
            )
        solo = (candidate_features[:, :10] - self.solo_mean) / self.solo_std
        bootstrap = self.solo_model(solo)
        if self.config.architecture_version == 1:
            context = candidate_features[:, 10:20] / self.context_scale
            return bootstrap + self.context_model(context).squeeze(-1)
        extra = candidate_features[:, 10:CANDIDATE_FEATURE_COUNT] / self.actor_extra_scale
        joint = torch.cat((solo, extra), dim=1)
        return bootstrap + self.residual_model(joint).squeeze(-1)

    def bootstrap_logits(self, candidate_features: Tensor) -> Tensor:
        if candidate_features.ndim != 2 or candidate_features.shape[1] < 10:
            raise ValueError(
                f"expected candidate features with solo prefix, got {candidate_features.shape}"
            )
        solo = (candidate_features[:, :10] - self.solo_mean) / self.solo_std
        return self.solo_model(solo)

    def value(self, state_features: Tensor) -> Tensor:
        required = (
            LEGACY_STATE_FEATURE_COUNT
            if self.config.architecture_version == 1
            else STATE_FEATURE_COUNT
        )
        if state_features.ndim != 2 or state_features.shape[1] < required:
            raise ValueError(
                f"expected [states, at least {required}], got {tuple(state_features.shape)}"
            )
        if self.config.architecture_version == 1:
            legacy = state_features[:, :LEGACY_STATE_FEATURE_COUNT]
            own = legacy[:, 0::2]
            opponent = legacy[:, 1::2]
            differences = (opponent - own) / self.state_scale
            return 0.5 * (
                self.value_core(differences).squeeze(-1) - self.value_core(-differences).squeeze(-1)
            )
        base = state_features[:, :12]
        own = (
            torch.cat(
                (
                    base[:, 0::2],
                    state_features[:, 12:22],
                    state_features[:, 32:42],
                    state_features[:, 52:87],
                ),
                dim=1,
            )
            / self.player_state_scale
        )
        opponent = (
            torch.cat(
                (
                    base[:, 1::2],
                    state_features[:, 22:32],
                    state_features[:, 42:52],
                    state_features[:, 87:122],
                ),
                dim=1,
            )
            / self.player_state_scale
        )
        forward = self.value_core(torch.cat((own, opponent), dim=1)).squeeze(-1)
        swapped = self.value_core(torch.cat((opponent, own), dim=1)).squeeze(-1)
        return 0.5 * (forward - swapped)

    def parameter_count(self) -> int:
        return sum(parameter.numel() for parameter in self.parameters())
