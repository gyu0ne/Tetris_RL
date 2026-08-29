from __future__ import annotations

import importlib
import threading
import time
from pathlib import Path
from typing import Protocol

import numpy as np
import torch

from tetris_rl.envs.versus import CANDIDATE_FEATURE_COUNT
from tetris_rl.models.versus_checkpoint import LoadedVersusActor, load_versus_actor


class HumanBattleBridge(Protocol):
    def bot_candidates(self) -> tuple[bytes, bool, bool]: ...

    def step(self, edges: list[tuple[str, str]], selection: int) -> None: ...

    def snapshot(self) -> tuple[object, ...]: ...


class HumanBattleController:
    def __init__(
        self,
        checkpoint: Path,
        seed: int,
        *,
        frames_per_placement: int = 12,
        allow_observed: bool,
        actor: LoadedVersusActor | None = None,
        bridge_factory: object | None = None,
    ) -> None:
        if seed < 0:
            raise ValueError("seed must be nonnegative")
        if frames_per_placement <= 0:
            raise ValueError("frames_per_placement must be positive")
        self._checkpoint = checkpoint
        self._actor = actor or load_versus_actor(
            checkpoint, allow_observed=allow_observed, device="cpu"
        )
        self._bridge_factory = bridge_factory or self._load_bridge_factory()
        self._frames_per_placement = frames_per_placement
        self._lock = threading.RLock()
        self._seed = seed
        self._bridge = self._new_bridge(seed)
        self._last_decision = self._empty_decision()

    def state(self) -> dict[str, object]:
        with self._lock:
            return self._state_unlocked()

    def reset(self, seed: int) -> dict[str, object]:
        if seed < 0:
            raise ValueError("seed must be nonnegative")
        with self._lock:
            self._seed = seed
            self._bridge = self._new_bridge(seed)
            self._last_decision = self._empty_decision()
            return self._state_unlocked()

    def step(self, edges: list[dict[str, object]]) -> dict[str, object]:
        parsed = self._parse_edges(edges)
        with self._lock, torch.inference_mode():
            raw_features, due, done = self._bridge.bot_candidates()
            selection = -1
            if due and not done:
                values = np.frombuffer(raw_features, dtype="<i4")
                if values.size == 0 or values.size % CANDIDATE_FEATURE_COUNT != 0:
                    raise RuntimeError("human battle bridge returned invalid candidate features")
                features = torch.from_numpy(values.copy().reshape(-1, CANDIDATE_FEATURE_COUNT)).to(
                    dtype=torch.float32
                )
                started = time.perf_counter()
                logits = self._actor.model.actor_logits(features)
                selection = int(torch.argmax(logits).item())
                elapsed_ms = (time.perf_counter() - started) * 1_000
                self._last_decision = {
                    "candidate_count": int(features.shape[0]),
                    "selected_index": selection,
                    "selected_score": float(logits[selection].item()),
                    "score_spread": float((logits.max() - logits.min()).item()),
                    "inference_ms": elapsed_ms,
                }
            if not done:
                self._bridge.step(parsed, selection)
            return self._state_unlocked()

    def _state_unlocked(self) -> dict[str, object]:
        frame, result, next_bot_frame, cadence, human, model = self._bridge.snapshot()
        metadata = self._actor.metadata
        return {
            "schema_version": "human-versus-model-v1",
            "seed": self._seed,
            "checkpoint": str(self._checkpoint),
            "parameters": sum(parameter.numel() for parameter in self._actor.model.parameters()),
            "engine_revision": metadata.get("engine_revision"),
            "training_update": metadata.get("update"),
            "frame": frame,
            "result": result,
            "next_bot_frame": next_bot_frame,
            "frames_per_placement": cadence,
            "human": self._player_state(human),
            "model": self._player_state(model),
            "last_decision": dict(self._last_decision),
        }

    @staticmethod
    def _player_state(snapshot: tuple[object, ...]) -> dict[str, object]:
        (
            board,
            garbage,
            active,
            hold,
            preview,
            pieces,
            pending,
            ready,
            sent,
            combo,
            back_to_back,
        ) = snapshot
        active_state = None
        if active is not None:
            kind, cells = active
            active_state = {"kind": kind, "cells": cells}
        return {
            "board_rows": board,
            "garbage_rows": garbage,
            "active": active_state,
            "hold": hold,
            "preview": preview,
            "pieces_placed": pieces,
            "pending_garbage": pending,
            "ready_garbage": ready,
            "sent_lines": sent,
            "combo": combo,
            "back_to_back": back_to_back,
        }

    @staticmethod
    def _parse_edges(edges: list[dict[str, object]]) -> list[tuple[str, str]]:
        if not isinstance(edges, list) or len(edges) > 64:
            raise ValueError("edges must be a list with at most 64 entries")
        parsed: list[tuple[str, str]] = []
        allowed_buttons = {
            "left",
            "right",
            "soft_drop",
            "hard_drop",
            "rotate_clockwise",
            "rotate_counterclockwise",
            "rotate_half",
            "hold",
        }
        for edge in edges:
            if not isinstance(edge, dict):
                raise ValueError("each edge must be an object")
            button = edge.get("button")
            kind = edge.get("kind")
            if button not in allowed_buttons or kind not in {"press", "release"}:
                raise ValueError("unknown input edge")
            parsed.append((str(button), str(kind)))
        return parsed

    def _new_bridge(self, seed: int) -> HumanBattleBridge:
        factory = self._bridge_factory
        if not callable(factory):
            raise TypeError("bridge factory must be callable")
        return factory(seed, self._frames_per_placement)

    @staticmethod
    def _load_bridge_factory() -> object:
        return importlib.import_module("tetris_engine").HumanBattle

    @staticmethod
    def _empty_decision() -> dict[str, int | float | None]:
        return {
            "candidate_count": 0,
            "selected_index": None,
            "selected_score": None,
            "score_spread": None,
            "inference_ms": None,
        }
