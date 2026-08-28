from dataclasses import dataclass
from pathlib import Path

import torch

from tetris_rl.features import MECHANICS_STATUS
from tetris_rl.models.afterstate import AfterstateScorer, ModelConfig
from tetris_rl.models.checkpoint import LoadedScorer
from tetris_rl.models.versus import VersusActorCritic, VersusModelConfig


@dataclass(frozen=True)
class LoadedVersusActor:
    model: VersusActorCritic
    metadata: dict[str, object]


def load_versus_actor(
    path: Path, *, allow_observed: bool = False, device: str = "cpu"
) -> LoadedVersusActor:
    payload = torch.load(path, map_location=device, weights_only=True)
    checkpoint_schema = payload.get("checkpoint_schema")
    _expect(
        checkpoint_schema in {"versus-actor-critic-v1", "versus-actor-critic-v2"},
        "checkpoint schema",
    )
    mechanics_status = payload.get("mechanics_status")
    if mechanics_status == MECHANICS_STATUS:
        _expect(allow_observed, "OBSERVED checkpoint requires explicit opt-in")
    else:
        _expect(mechanics_status == "CONFORMANT", "mechanics status")
    solo_config = ModelConfig(**payload["solo_model_config"])
    solo = LoadedScorer(
        model=AfterstateScorer(solo_config),
        feature_mean=torch.zeros(solo_config.input_features, device=device),
        feature_std=torch.ones(solo_config.input_features, device=device),
        metadata={},
    )
    model_config = dict(payload["model_config"])
    if checkpoint_schema == "versus-actor-critic-v1":
        model_config["architecture_version"] = 1
    model = VersusActorCritic(solo, VersusModelConfig(**model_config)).to(device)
    model.load_state_dict(payload["model_state"], strict=True)
    model.eval()
    return LoadedVersusActor(model=model, metadata=payload)


def _expect(condition: bool, label: str) -> None:
    if not condition:
        raise ValueError(f"invalid versus checkpoint: {label}")
