from __future__ import annotations

import importlib
import threading
import time
from pathlib import Path
from typing import Protocol

import numpy as np
import torch

from tetris_rl.features import FEATURE_NAMES
from tetris_rl.models.checkpoint import LoadedScorer, load_scorer


class SpectatorBridge(Protocol):
    def candidates(self) -> tuple[bytes, list[int], list[bool]]: ...

    def step(self, selections: list[int]) -> None: ...

    def snapshot(
        self, index: int
    ) -> tuple[list[int], list[int], str, str | None, list[str], int, bool]: ...


class SpectatorController:
    def __init__(
        self,
        checkpoint: Path,
        seed: int,
        *,
        allow_observed: bool,
        scorer: LoadedScorer | None = None,
        bridge_factory: object | None = None,
    ) -> None:
        if seed < 0:
            raise ValueError("seed must be nonnegative")
        self._checkpoint = checkpoint
        self._scorer = scorer or load_scorer(checkpoint, allow_observed=allow_observed)
        self._bridge_factory = bridge_factory or self._load_bridge_factory()
        self._lock = threading.RLock()
        self._seed = seed
        self._bridge = self._new_bridge(seed)
        self._last_decision: dict[str, int | float | None] = {
            "candidate_count": 0,
            "selected_index": None,
            "selected_score": None,
            "score_spread": None,
            "inference_ms": None,
        }

    def state(self) -> dict[str, object]:
        with self._lock:
            return self._state_unlocked()

    def reset(self, seed: int) -> dict[str, object]:
        if seed < 0:
            raise ValueError("seed must be nonnegative")
        with self._lock:
            self._seed = seed
            self._bridge = self._new_bridge(seed)
            self._last_decision = {
                "candidate_count": 0,
                "selected_index": None,
                "selected_score": None,
                "score_spread": None,
                "inference_ms": None,
            }
            return self._state_unlocked()

    def step(self, count: int) -> dict[str, object]:
        if not 1 <= count <= 200:
            raise ValueError("count must be between 1 and 200")
        with self._lock, torch.inference_mode():
            for _ in range(count):
                raw_features, offsets, done = self._bridge.candidates()
                if len(offsets) != 2 or len(done) != 1 or offsets[0] != 0:
                    raise RuntimeError("spectator bridge returned invalid single-game offsets")
                if done[0]:
                    break
                candidate_count = offsets[1]
                values = np.frombuffer(raw_features, dtype="<i4")
                expected = candidate_count * len(FEATURE_NAMES)
                if values.size != expected:
                    raise RuntimeError(
                        f"spectator bridge returned {values.size} values, expected {expected}"
                    )
                features = torch.from_numpy(
                    values.copy().reshape(candidate_count, len(FEATURE_NAMES))
                ).to(dtype=torch.float32)
                started = time.perf_counter()
                scores = self._scorer.score(features)
                selected = int(torch.argmax(scores).item())
                elapsed_ms = (time.perf_counter() - started) * 1_000
                self._bridge.step([selected])
                self._last_decision = {
                    "candidate_count": candidate_count,
                    "selected_index": selected,
                    "selected_score": float(scores[selected].item()),
                    "score_spread": float((scores.max() - scores.min()).item()),
                    "inference_ms": elapsed_ms,
                }
            return self._state_unlocked()

    def _state_unlocked(self) -> dict[str, object]:
        board, garbage, active, hold, preview, pieces, top_out = self._bridge.snapshot(0)
        metadata = self._scorer.metadata
        return {
            "schema_version": "solo-model-spectator-v1",
            "seed": self._seed,
            "checkpoint": str(self._checkpoint),
            "parameters": self._scorer.model.parameter_count(),
            "engine_revision": metadata.get("engine_revision"),
            "dataset_id": metadata.get("dataset_id"),
            "board_rows": board,
            "garbage_rows": garbage,
            "active": active,
            "hold": hold,
            "preview": preview,
            "pieces_placed": pieces,
            "top_out": top_out,
            "last_decision": dict(self._last_decision),
        }

    def _new_bridge(self, seed: int) -> SpectatorBridge:
        factory = self._bridge_factory
        if not callable(factory):
            raise TypeError("bridge factory must be callable")
        return factory([seed])

    @staticmethod
    def _load_bridge_factory() -> object:
        module = importlib.import_module("tetris_engine")
        return module.SoloBatch
