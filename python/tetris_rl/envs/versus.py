from dataclasses import dataclass
from importlib import import_module
from typing import Protocol

import torch
from torch import Tensor

CANDIDATE_FEATURE_COUNT = 20
CANDIDATE_DIAGNOSTIC_COUNT = 5
STATE_FEATURE_COUNT = 12


class VersusBridge(Protocol):
    def candidates(self) -> tuple[bytes, bytes, bytes, list[int], list[bool], list[int]]: ...

    def step(self, selections: list[int]) -> None: ...

    def reset_done(self, seeds: list[int]) -> None: ...

    def match_count(self) -> int: ...


@dataclass(frozen=True)
class VersusObservation:
    candidate_features: Tensor
    candidate_diagnostics: Tensor
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
        self._frames_per_placement = frames_per_placement
        self._seeds = list(seeds)
        self._histories: list[list[tuple[int, int]]] = [[] for _ in seeds]
        self._last_done = [False] * (len(seeds) * 2)

    @classmethod
    def restore(cls, state: dict[str, object]) -> "VersusVectorEnv":
        if state.get("schema_version") != "versus-vector-env-history-v1":
            raise ValueError("unsupported versus environment state")
        seeds = [int(seed) for seed in state["seeds"]]  # type: ignore[index]
        histories = [
            [(int(one), int(two)) for one, two in history]
            for history in state["histories"]  # type: ignore[index]
        ]
        frames_per_placement = int(state["frames_per_placement"])
        extension = import_module("tetris_engine")
        bridge = extension.VersusBatch.restore(seeds, histories, frames_per_placement)
        env = cls(seeds, frames_per_placement, bridge=bridge)
        env._histories = histories
        return env

    @property
    def match_count(self) -> int:
        return int(self._bridge.match_count())

    def observe(self) -> VersusObservation:
        candidate_bytes, diagnostic_bytes, state_bytes, offsets, done, results = (
            self._bridge.candidates()
        )
        candidate_features = _decode_i32(candidate_bytes, CANDIDATE_FEATURE_COUNT)
        candidate_diagnostics = _decode_i32(diagnostic_bytes, CANDIDATE_DIAGNOSTIC_COUNT)
        state_features = _decode_i32(state_bytes, STATE_FEATURE_COUNT)
        if len(offsets) != len(done) + 1 or len(done) != len(results):
            raise ValueError("invalid versus bridge batch dimensions")
        if state_features.shape[0] != len(done):
            raise ValueError("state feature count differs from decision count")
        if offsets[-1] != candidate_features.shape[0]:
            raise ValueError("candidate offsets do not cover the feature buffer")
        if candidate_diagnostics.shape[0] != candidate_features.shape[0]:
            raise ValueError("candidate diagnostics differ from feature count")
        observation = VersusObservation(
            candidate_features=candidate_features,
            candidate_diagnostics=candidate_diagnostics,
            state_features=state_features,
            offsets=tuple(int(value) for value in offsets),
            done=torch.tensor(done, dtype=torch.bool),
            results=torch.tensor(results, dtype=torch.float32),
        )
        self._last_done = [bool(value) for value in done]
        return observation

    def step(self, selections: list[int | None]) -> VersusObservation:
        if len(selections) != self.match_count * 2:
            raise ValueError("selection count differs from environment decisions")
        active_pairs: list[tuple[int, tuple[int, int]]] = []
        for match_index in range(self.match_count):
            one, two = selections[match_index * 2 : match_index * 2 + 2]
            if self._last_done[match_index * 2]:
                if one is not None or two is not None:
                    raise ValueError("completed match received a selection")
            else:
                if one is None or two is None:
                    raise ValueError("active match is missing a selection")
                active_pairs.append((match_index, (int(one), int(two))))
        encoded = [-1 if selection is None else int(selection) for selection in selections]
        self._bridge.step(encoded)
        for match_index, pair in active_pairs:
            self._histories[match_index].append(pair)
        return self.observe()

    def reset_done(self, seeds: list[int]) -> VersusObservation:
        completed = [index for index in range(self.match_count) if self._last_done[index * 2]]
        if len(seeds) != len(completed):
            raise ValueError("reset seed count differs from completed matches")
        self._bridge.reset_done([int(seed) for seed in seeds])
        for match_index, seed in zip(completed, seeds, strict=True):
            self._seeds[match_index] = int(seed)
            self._histories[match_index] = []
        return self.observe()

    def state_dict(self) -> dict[str, object]:
        return {
            "schema_version": "versus-vector-env-history-v1",
            "frames_per_placement": self._frames_per_placement,
            "seeds": list(self._seeds),
            "histories": [list(history) for history in self._histories],
        }


def _decode_i32(payload: bytes, width: int) -> Tensor:
    values = torch.frombuffer(bytearray(payload), dtype=torch.int32)
    if values.numel() % width != 0:
        raise ValueError(f"buffer length is not divisible by feature width {width}")
    return values.reshape(-1, width).to(dtype=torch.float32)
