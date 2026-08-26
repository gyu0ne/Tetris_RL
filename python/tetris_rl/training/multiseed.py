from __future__ import annotations

import argparse
import json
from concurrent.futures import ProcessPoolExecutor, as_completed
from multiprocessing import get_context
from pathlib import Path

import torch

from tetris_rl.features import FEATURE_NAMES
from tetris_rl.training.imitation import (
    PreparedTrainingData,
    _training_config,
    prepare_training_data,
    train,
)


def main() -> None:
    parser = argparse.ArgumentParser(description="Train independent imitation candidates")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--seeds", type=int, nargs="+", default=[2026, 2027, 2028])
    parser.add_argument("--epochs", type=int, default=100, help="maximum epochs")
    parser.add_argument("--min-epochs", type=int, default=20)
    parser.add_argument("--patience", type=int, default=10)
    parser.add_argument("--min-improvement", type=float, default=0.1)
    parser.add_argument("--shuffle-buffer", type=int, default=4_096)
    parser.add_argument("--batch-decisions", type=int, default=64)
    parser.add_argument("--learning-rate", type=float, default=3.0e-4)
    parser.add_argument("--teacher-temperature", type=float, default=1.0)
    parser.add_argument("--teacher-score-scale", type=float, default=1_000.0)
    parser.add_argument("--threads", type=int, default=2, help="PyTorch threads per worker")
    parser.add_argument("--workers", type=int, default=1, help="parallel initialization workers")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--allow-observed", action="store_true")
    args = parser.parse_args()
    if len(args.seeds) < 3 or len(set(args.seeds)) != len(args.seeds):
        raise ValueError("final bootstrap needs at least three unique initialization seeds")
    if args.workers <= 0:
        raise ValueError("workers must be positive")

    prepared = prepare_training_data(args.manifest, allow_observed=args.allow_observed)
    outputs = {seed: args.output_dir / f"seed-{seed}.pt" for seed in args.seeds}
    pending: list[int] = []
    for seed in args.seeds:
        candidate_args = _candidate_args(args, seed, outputs[seed])
        if args.resume and outputs[seed].is_file():
            _validate_completed_checkpoint(outputs[seed], prepared, candidate_args)
            candidate_args.progress_checkpoint.unlink(missing_ok=True)
            print(
                json.dumps(
                    {
                        "event": "completed_candidate_reused",
                        "seed": seed,
                        "checkpoint": str(outputs[seed]),
                    },
                    sort_keys=True,
                ),
                flush=True,
            )
        else:
            pending.append(seed)

    worker_count = min(args.workers, len(pending))
    if worker_count == 1:
        for seed in pending:
            _train_candidate(_candidate_args(args, seed, outputs[seed]), prepared)
    elif worker_count > 1:
        context = get_context("spawn")
        with ProcessPoolExecutor(max_workers=worker_count, mp_context=context) as executor:
            futures = {
                executor.submit(
                    _train_candidate,
                    _candidate_args(args, seed, outputs[seed]),
                    prepared,
                ): seed
                for seed in pending
            }
            for future in as_completed(futures):
                future.result()

    print(
        json.dumps(
            {
                "schema_version": "imitation-multiseed-training-v2",
                "dataset_id": prepared.dataset.manifest["dataset_id"],
                "initialization_seeds": args.seeds,
                "workers": args.workers,
                "threads_per_worker": args.threads,
                "resume": args.resume,
                "checkpoints": [str(outputs[seed]) for seed in args.seeds],
            },
            sort_keys=True,
        ),
        flush=True,
    )


def _candidate_args(args: argparse.Namespace, seed: int, output: Path) -> argparse.Namespace:
    return argparse.Namespace(
        manifest=args.manifest,
        output=output,
        progress_checkpoint=output.with_name(f"{output.stem}.progress.pt"),
        epochs=args.epochs,
        min_epochs=args.min_epochs,
        patience=args.patience,
        min_improvement=args.min_improvement,
        shuffle_buffer=args.shuffle_buffer,
        batch_decisions=args.batch_decisions,
        learning_rate=args.learning_rate,
        teacher_temperature=args.teacher_temperature,
        teacher_score_scale=args.teacher_score_scale,
        seed=seed,
        threads=args.threads,
        resume=args.resume,
        allow_observed=args.allow_observed,
    )


def _train_candidate(args: argparse.Namespace, prepared: PreparedTrainingData) -> str:
    return str(train(args, prepared))


def _validate_completed_checkpoint(
    path: Path,
    prepared: PreparedTrainingData,
    args: argparse.Namespace,
) -> None:
    payload = torch.load(path, map_location="cpu", weights_only=True)
    if payload.get("checkpoint_schema") != "afterstate-scorer-v1":
        raise ValueError(f"completed checkpoint has unsupported schema: {path}")
    if payload.get("dataset_id") != prepared.dataset.manifest["dataset_id"]:
        raise ValueError(f"completed checkpoint uses a different dataset: {path}")
    if tuple(payload.get("feature_names", ())) != FEATURE_NAMES:
        raise ValueError(f"completed checkpoint uses a different feature contract: {path}")
    training = payload.get("training")
    if not isinstance(training, dict):
        raise ValueError(f"completed checkpoint has no training metadata: {path}")
    for key, expected in _training_config(args).items():
        if training.get(key) != expected:
            raise ValueError(f"completed checkpoint training setting differs for {key}: {path}")
    completed_epochs = training.get("epochs")
    history = payload.get("training_history")
    if (
        not isinstance(completed_epochs, int)
        or completed_epochs < args.min_epochs
        or not isinstance(history, list)
        or len(history) != completed_epochs
    ):
        raise ValueError(f"completed checkpoint is incomplete: {path}")


if __name__ == "__main__":
    main()
