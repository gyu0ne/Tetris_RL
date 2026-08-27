from dataclasses import dataclass
from importlib import import_module
from typing import Protocol

import torch
from torch import Tensor

CANDIDATE_FEATURE_COUNT = 20
STATE_FEATURE_COUNT = 12


class VersusBridge(Protocol):
    def candidates(self) -> tuple[bytes, bytes, list[int], list[bool], list[int]]: ...

    def step(self, selections: list[int]) -> None: ...

    def reset_done(self, seeds: list[int]) -> None: ...

    def match_count(self) -> int: ...


@dataclass(frozen=True)
class VersusObservation:
    candidate_features: Tensor
    state_features: Tensor
    offsets: tuple[int, ...]
    done: Tensor
    results: Tensor

    @property
    def decision_count(self) -> int:
        return len(self.offsets) - 1


class VersusVectorEnv:
    def __init__(
        self,
        seeds: list[int],
        frames_per_placement: int = 12,
        *,
        bridge: VersusBridge | None = None,
    ) -> None:
        if not seeds:
            raise ValueError("at least one seed is required")
        if frames_per_placement <= 0:
            raise ValueError("frames_per_placement must be positive")
        if bridge is None:
            extension = import_module("tetris_engine")
            bridge = extension.VersusBatch(seeds, frames_per_placement)
        self._bridge = bridge

    @property
    def match_count(self) -> int:
        return int(self._bridge.match_count())

    def observe(self) -> VersusObservation:
        candidate_bytes, state_bytes, offsets, done, results = self._bridge.candidates()
        candidate_features = _decode_i32(candidate_bytes, CANDIDATE_FEATURE_COUNT)
        state_features = _decode_i32(state_bytes, STATE_FEATURE_COUNT)
        if len(offsets) != len(done) + 1 or len(done) != len(results):
            raise ValueError("invalid versus bridge batch dimensions")
        if state_features.shape[0] != len(done):
            raise ValueError("state feature count differs from decision count")
        if offsets[-1] != candidate_features.shape[0]:
            raise ValueError("candidate offsets do not cover the feature buffer")
        return VersusObservation(
            candidate_features=candidate_features,
            state_features=state_features,
            offsets=tuple(int(value) for value in offsets),
            done=torch.tensor(done, dtype=torch.bool),
            results=torch.tensor(results, dtype=torch.float32),
        )

    def step(self, selections: list[int | None]) -> VersusObservation:
        encoded = [-1 if selection is None else int(selection) for selection in selections]
        self._bridge.step(encoded)
        return self.observe()

    def reset_done(self, seeds: list[int]) -> VersusObservation:
        self._bridge.reset_done([int(seed) for seed in seeds])
        return self.observe()


def _decode_i32(payload: bytes, width: int) -> Tensor:
    values = torch.frombuffer(bytearray(payload), dtype=torch.int32)
    if values.numel() % width != 0:
        raise ValueError(f"buffer length is not divisible by feature width {width}")
    return values.reshape(-1, width).to(dtype=torch.float32)
