from __future__ import annotations

import argparse
import json
from pathlib import Path

from tetris_rl.training.imitation import prepare_training_data, train


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
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--allow-observed", action="store_true")
    args = parser.parse_args()
    if len(args.seeds) < 3 or len(set(args.seeds)) != len(args.seeds):
        raise ValueError("final bootstrap needs at least three unique initialization seeds")

    prepared = prepare_training_data(args.manifest, allow_observed=args.allow_observed)
    outputs = []
    for seed in args.seeds:
        output = args.output_dir / f"seed-{seed}.pt"
        train(
            argparse.Namespace(
                manifest=args.manifest,
                output=output,
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
                allow_observed=args.allow_observed,
            ),
            prepared,
        )
        outputs.append(str(output))
    print(
        json.dumps(
            {
                "schema_version": "imitation-multiseed-training-v1",
                "dataset_id": prepared.dataset.manifest["dataset_id"],
                "initialization_seeds": args.seeds,
                "checkpoints": outputs,
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
