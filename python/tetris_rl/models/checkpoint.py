from dataclasses import dataclass
from pathlib import Path

import torch
from torch import Tensor

from tetris_rl.features import FEATURE_NAMES, MECHANICS_STATUS, SCHEMA_VERSION
from tetris_rl.models.afterstate import AfterstateScorer, ModelConfig


@dataclass(frozen=True)
class LoadedScorer:
    model: AfterstateScorer
    feature_mean: Tensor
    feature_std: Tensor
    metadata: dict[str, object]

    def score(self, features: Tensor) -> Tensor:
        normalized = (features - self.feature_mean) / self.feature_std
        with torch.no_grad():
            return self.model(normalized)


def load_scorer(path: Path, *, allow_observed: bool = False, device: str = "cpu") -> LoadedScorer:
    payload = torch.load(path, map_location=device, weights_only=True)
    _expect(payload.get("checkpoint_schema") == "afterstate-scorer-v1", "checkpoint schema")
    _expect(payload.get("dataset_schema") == SCHEMA_VERSION, "dataset schema")
    _expect(tuple(payload.get("feature_names", ())) == FEATURE_NAMES, "feature contract")
    mechanics_status = payload.get("mechanics_status")
    if mechanics_status == MECHANICS_STATUS:
        _expect(allow_observed, "OBSERVED checkpoint requires explicit opt-in")
    else:
        _expect(mechanics_status == "CONFORMANT", "mechanics status")

    config = ModelConfig(**payload["model_config"])
    model = AfterstateScorer(config).to(device)
    model.load_state_dict(payload["model_state"], strict=True)
    model.eval()
    mean = torch.tensor(payload["feature_mean"], dtype=torch.float32, device=device)
    std = torch.tensor(payload["feature_std"], dtype=torch.float32, device=device)
    _expect(tuple(mean.shape) == (len(FEATURE_NAMES),), "feature mean shape")
    _expect(tuple(std.shape) == (len(FEATURE_NAMES),), "feature std shape")
    _expect(bool(torch.all(std > 0)), "feature std must be positive")
    return LoadedScorer(model, mean, std, payload)


def _expect(condition: bool, label: str) -> None:
    if not condition:
        raise ValueError(f"invalid scorer checkpoint: {label}")
