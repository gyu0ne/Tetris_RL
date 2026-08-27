from __future__ import annotations

import argparse
import importlib
import json
import multiprocessing
from concurrent.futures import ProcessPoolExecutor
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
    parser.add_argument("--workers", type=int, default=1)
    parser.add_argument("--allow-observed", action="store_true")
    parser.add_argument("--require-gates", action="store_true")
    parser.add_argument("--min-survival", type=float, default=0.95)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if (
        args.base_seed < 0
        or args.seeds <= 0
        or args.horizon <= 0
        or args.threads <= 0
        or args.workers <= 0
    ):
        raise ValueError(
            "base-seed must be nonnegative; seeds, horizon, threads and workers must be positive"
        )
    if not 0.0 <= args.min_survival <= 1.0:
        raise ValueError("min-survival must be in [0, 1]")

    scorer = load_scorer(args.checkpoint, allow_observed=args.allow_observed)
    evaluation_seeds = list(range(args.base_seed, args.base_seed + args.seeds))
    metrics = evaluate_checkpoint_parallel(
        args.checkpoint,
        evaluation_seeds,
        args.horizon,
        workers=args.workers,
        threads_per_worker=args.threads,
        allow_observed=args.allow_observed,
        loaded_scorer=scorer,
    )
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


def evaluate_checkpoint_parallel(
    checkpoint: Path,
    seeds: list[int],
    horizon: int,
    *,
    workers: int,
    threads_per_worker: int,
    allow_observed: bool,
    loaded_scorer: LoadedScorer | None = None,
) -> ClosedLoopMetrics:
    if not seeds or horizon <= 0 or workers <= 0 or threads_per_worker <= 0:
        raise ValueError("seeds, horizon, workers and threads_per_worker must be positive")
    worker_count = min(workers, len(seeds))
    if worker_count == 1:
        torch.set_num_threads(threads_per_worker)
        scorer = loaded_scorer or load_scorer(checkpoint, allow_observed=allow_observed)
        return evaluate_closed_loop(scorer, _new_bridge(seeds), horizon)

    chunks = _partition_seeds(seeds, worker_count)
    context = multiprocessing.get_context("spawn")
    with ProcessPoolExecutor(max_workers=worker_count, mp_context=context) as executor:
        futures = [
            executor.submit(
                _evaluate_checkpoint_chunk,
                checkpoint,
                chunk,
                horizon,
                threads_per_worker,
                allow_observed,
            )
            for chunk in chunks
        ]
        metrics = [future.result() for future in futures]
    return combine_closed_loop_metrics(metrics)


def combine_closed_loop_metrics(metrics: list[ClosedLoopMetrics]) -> ClosedLoopMetrics:
    if not metrics:
        raise ValueError("at least one closed-loop metric shard is required")
    horizon = metrics[0].horizon
    if any(metric.horizon != horizon for metric in metrics):
        raise ValueError("closed-loop metric shards must share one horizon")
    seeds = sum(metric.seeds for metric in metrics)
    if seeds <= 0:
        raise ValueError("closed-loop metric shards must contain seeds")
    survived = sum(metric.survived for metric in metrics)
    total_pieces = sum(metric.mean_pieces_placed * metric.seeds for metric in metrics)
    return ClosedLoopMetrics(
        seeds=seeds,
        horizon=horizon,
        survived=survived,
        survival_at_horizon=survived / seeds,
        mean_pieces_placed=total_pieces / seeds,
        min_pieces_placed=min(metric.min_pieces_placed for metric in metrics),
        max_pieces_placed=max(metric.max_pieces_placed for metric in metrics),
    )


def _partition_seeds(seeds: list[int], workers: int) -> list[list[int]]:
    chunk_size = (len(seeds) + workers - 1) // workers
    return [seeds[start : start + chunk_size] for start in range(0, len(seeds), chunk_size)]


def _evaluate_checkpoint_chunk(
    checkpoint: Path,
    seeds: list[int],
    horizon: int,
    threads_per_worker: int,
    allow_observed: bool,
) -> ClosedLoopMetrics:
    torch.set_num_threads(threads_per_worker)
    scorer = load_scorer(checkpoint, allow_observed=allow_observed)
    return evaluate_closed_loop(scorer, _new_bridge(seeds), horizon)


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
