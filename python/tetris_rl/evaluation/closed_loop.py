from __future__ import annotations

import argparse
import importlib
import json
from dataclasses import asdict, dataclass
from hashlib import sha256
from pathlib import Path
from typing import Protocol

import numpy as np
import torch

from tetris_rl.features import FEATURE_NAMES
from tetris_rl.models import load_scorer
from tetris_rl.models.checkpoint import LoadedScorer


class SoloBatchBridge(Protocol):
    def candidates(self) -> tuple[bytes, list[int], list[bool]]: ...

    def step(self, selections: list[int]) -> None: ...

    def pieces_placed(self) -> list[int]: ...


@dataclass(frozen=True)
class ClosedLoopMetrics:
    seeds: int
    horizon: int
    survived: int
    survival_at_horizon: float
    mean_pieces_placed: float
    min_pieces_placed: int
    max_pieces_placed: int


def main() -> None:
    parser = argparse.ArgumentParser(description="Run a scorer in the authoritative solo engine")
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--base-seed", type=int, default=20_001)
    parser.add_argument("--seeds", type=int, default=500)
    parser.add_argument("--horizon", type=int, default=1_000)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--allow-observed", action="store_true")
    parser.add_argument("--require-gates", action="store_true")
    parser.add_argument("--min-survival", type=float, default=0.95)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.base_seed < 0 or args.seeds <= 0 or args.horizon <= 0 or args.threads <= 0:
        raise ValueError(
            "base-seed must be nonnegative; seeds, horizon and threads must be positive"
        )
    if not 0.0 <= args.min_survival <= 1.0:
        raise ValueError("min-survival must be in [0, 1]")

    torch.set_num_threads(args.threads)
    scorer = load_scorer(args.checkpoint, allow_observed=args.allow_observed)
    bridge = _new_bridge(list(range(args.base_seed, args.base_seed + args.seeds)))
    metrics = evaluate_closed_loop(scorer, bridge, args.horizon)
    passed = metrics.survival_at_horizon >= args.min_survival
    report = {
        "schema_version": "closed-loop-solo-evaluation-v1",
        "checkpoint": str(args.checkpoint),
        "checkpoint_sha256": _sha256_file(args.checkpoint),
        "dataset_id": scorer.metadata["dataset_id"],
        "engine_revision": scorer.metadata["engine_revision"],
        "base_seed": args.base_seed,
        "metrics": asdict(metrics),
        "thresholds": {"min_survival": args.min_survival},
        "gates": {"survival_at_horizon": passed, "passed": passed},
    }
    serialized = json.dumps(report, sort_keys=True)
    print(serialized)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(serialized + "\n", encoding="utf-8")
    if args.require_gates and not passed:
        raise SystemExit(3)


def evaluate_closed_loop(
    scorer: LoadedScorer,
    bridge: SoloBatchBridge,
    horizon: int,
) -> ClosedLoopMetrics:
    for _ in range(horizon):
        raw_features, offsets, done = bridge.candidates()
        if len(offsets) != len(done) + 1 or offsets[0] != 0:
            raise ValueError("invalid engine candidate offsets")
        if all(done):
            break
        candidate_count = offsets[-1]
        values = np.frombuffer(raw_features, dtype="<i4")
        expected_values = candidate_count * len(FEATURE_NAMES)
        if values.size != expected_values:
            raise ValueError(
                f"engine returned {values.size} feature values, expected {expected_values}"
            )
        features = torch.from_numpy(values.copy().reshape(candidate_count, len(FEATURE_NAMES)))
        logits = scorer.score(features.to(dtype=torch.float32))
        selections = [-1] * len(done)
        for game, is_done in enumerate(done):
            start = offsets[game]
            end = offsets[game + 1]
            if is_done:
                if start != end:
                    raise ValueError("done engine game returned candidates")
                continue
            if start == end:
                raise ValueError("active engine game returned no candidates")
            selections[game] = int(torch.argmax(logits[start:end]).item())
        bridge.step(selections)

    pieces = bridge.pieces_placed()
    if not pieces:
        raise ValueError("closed-loop evaluation needs at least one seed")
    survived = sum(count >= horizon for count in pieces)
    return ClosedLoopMetrics(
        seeds=len(pieces),
        horizon=horizon,
        survived=survived,
        survival_at_horizon=survived / len(pieces),
        mean_pieces_placed=sum(pieces) / len(pieces),
        min_pieces_placed=min(pieces),
        max_pieces_placed=max(pieces),
    )


def _new_bridge(seeds: list[int]) -> SoloBatchBridge:
    module = importlib.import_module("tetris_engine")
    return module.SoloBatch(seeds)


def _sha256_file(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


if __name__ == "__main__":
    main()
