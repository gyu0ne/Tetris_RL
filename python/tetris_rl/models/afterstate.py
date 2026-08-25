from dataclasses import asdict, dataclass

from torch import Tensor, nn


@dataclass(frozen=True)
class ModelConfig:
    input_features: int = 10
    hidden_one: int = 64
    hidden_two: int = 32

    def to_dict(self) -> dict[str, int]:
        return asdict(self)


class AfterstateScorer(nn.Module):
    """Shared scalar scorer applied independently to every legal afterstate."""

    def __init__(self, config: ModelConfig | None = None) -> None:
        super().__init__()
        self.config = config or ModelConfig()
        self.network = nn.Sequential(
            nn.Linear(self.config.input_features, self.config.hidden_one),
            nn.ReLU(),
            nn.Linear(self.config.hidden_one, self.config.hidden_two),
            nn.ReLU(),
            nn.Linear(self.config.hidden_two, 1),
        )

    def forward(self, features: Tensor) -> Tensor:
        if features.ndim != 2 or features.shape[1] != self.config.input_features:
            raise ValueError(
                f"expected [candidates, {self.config.input_features}], got {tuple(features.shape)}"
            )
        return self.network(features).squeeze(-1)

    def parameter_count(self) -> int:
        return sum(parameter.numel() for parameter in self.parameters())
