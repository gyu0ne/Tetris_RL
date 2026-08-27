from __future__ import annotations

import argparse
import json
import shutil
from dataclasses import asdict
from hashlib import sha256
from pathlib import Path

import torch

from tetris_rl.evaluation.closed_loop import evaluate_checkpoint_parallel
from tetris_rl.evaluation.offline import evaluate_decisions
from tetris_rl.models import load_scorer
from tetris_rl.training.dataset import iter_decisions, validate_dataset


def main() -> None:
    parser = argparse.ArgumentParser(description="Select one imitation run on paired gates")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, action="append", required=True)
    parser.add_argument("--output-checkpoint", type=Path, required=True)
    parser.add_argument("--offline-output", type=Path, required=True)
    parser.add_argument("--selection-output", type=Path, required=True)
    parser.add_argument("--base-seed", type=int, default=20_001)
    parser.add_argument("--seeds", type=int, default=256)
    parser.add_argument("--horizon", type=int, default=2_000)
    parser.add_argument("--batch-decisions", type=int, default=64)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--workers", type=int, default=1)
    parser.add_argument("--min-tie-aware", type=float, default=0.97)
    parser.add_argument("--min-positive-margin", type=float, default=0.95)
    parser.add_argument("--max-normalized-regret", type=float, default=0.05)
    parser.add_argument("--min-dev-survival", type=float, default=1.0)
    parser.add_argument("--allow-observed", action="store_true")
    args = parser.parse_args()
    if len(args.candidate) < 2:
        raise ValueError("selection needs at least two independently initialized candidates")
    if len({path.resolve() for path in args.candidate}) != len(args.candidate):
        raise ValueError("candidate paths must be unique")
    if args.base_seed < 0 or args.seeds <= 0 or args.horizon <= 0:
        raise ValueError("base-seed must be nonnegative; seeds and horizon must be positive")
    if args.batch_decisions <= 0 or args.threads <= 0 or args.workers <= 0:
        raise ValueError("batch-decisions, threads and workers must be positive")

    torch.set_num_threads(args.threads)
    dataset = validate_dataset(args.manifest, allow_observed=args.allow_observed)
    evaluation_seeds = list(range(args.base_seed, args.base_seed + args.seeds))
    _ensure_disjoint(dataset.manifest, evaluation_seeds)

    candidates: list[dict[str, object]] = []
    for path in args.candidate:
        scorer = load_scorer(path, allow_observed=args.allow_observed)
        if scorer.metadata.get("dataset_id") != dataset.manifest.get("dataset_id"):
            raise ValueError(f"candidate dataset_id differs: {path}")
        if scorer.metadata.get("engine_revision") != dataset.manifest.get("engine_revision"):
            raise ValueError(f"candidate engine_revision differs: {path}")

        offline = evaluate_decisions(
            scorer,
            iter_decisions(dataset, "validation"),
            args.batch_decisions,
        )
        normalized = offline.mean_normalized_regret
        offline_gates = {
            "tie_aware_optimal_rate": offline.tie_aware_optimal_rate >= args.min_tie_aware,
            "positive_margin_agreement": (
                offline.positive_margin_agreement >= args.min_positive_margin
            ),
            "mean_normalized_regret": (
                normalized is not None and normalized <= args.max_normalized_regret
            ),
        }
        closed_loop = evaluate_checkpoint_parallel(
            path,
            evaluation_seeds,
            args.horizon,
            workers=args.workers,
            threads_per_worker=args.threads,
            allow_observed=args.allow_observed,
            loaded_scorer=scorer,
        )
        dev_gate = closed_loop.survival_at_horizon >= args.min_dev_survival
        training = scorer.metadata.get("training", {})
        candidates.append(
            {
                "checkpoint": str(path),
                "checkpoint_sha256": _sha256_file(path),
                "training_seed": training.get("seed") if isinstance(training, dict) else None,
                "selected_epoch": (
                    training.get("selected_epoch") if isinstance(training, dict) else None
                ),
                "offline": asdict(offline),
                "offline_gates": {**offline_gates, "passed": all(offline_gates.values())},
                "development_closed_loop": asdict(closed_loop),
                "development_gate": {"survival_at_horizon": dev_gate, "passed": dev_gate},
                "eligible": all(offline_gates.values()) and dev_gate,
            }
        )

    selected = _choose_candidate(candidates)
    report = {
        "schema_version": "imitation-candidate-selection-v1",
        "dataset_id": dataset.manifest["dataset_id"],
        "engine_revision": dataset.manifest["engine_revision"],
        "development_seed_schedule": {
            "base_seed": args.base_seed,
            "seeds": args.seeds,
            "horizon": args.horizon,
        },
        "thresholds": {
            "min_tie_aware": args.min_tie_aware,
            "min_positive_margin": args.min_positive_margin,
            "max_normalized_regret": args.max_normalized_regret,
            "min_dev_survival": args.min_dev_survival,
        },
        "candidates": candidates,
        "selected_checkpoint_sha256": (
            selected["checkpoint_sha256"] if selected is not None else None
        ),
        "passed": selected is not None,
    }
    _write_json(args.selection_output, report)
    print(json.dumps(report, sort_keys=True))
    if selected is None:
        raise SystemExit(3)

    selected_path = Path(str(selected["checkpoint"]))
    args.output_checkpoint.parent.mkdir(parents=True, exist_ok=True)
    if selected_path.resolve() != args.output_checkpoint.resolve():
        shutil.copyfile(selected_path, args.output_checkpoint)
    selected_hash = _sha256_file(args.output_checkpoint)
    if selected_hash != selected["checkpoint_sha256"]:
        raise RuntimeError("selected checkpoint copy changed SHA-256")

    offline_report = {
        "schema_version": "offline-imitation-evaluation-v1",
        "checkpoint": str(args.output_checkpoint),
        "checkpoint_sha256": selected_hash,
        "dataset_id": dataset.manifest["dataset_id"],
        "engine_revision": dataset.manifest["engine_revision"],
        "split": "validation",
        "metrics": selected["offline"],
        "thresholds": {
            "min_tie_aware": args.min_tie_aware,
            "min_positive_margin": args.min_positive_margin,
            "max_normalized_regret": args.max_normalized_regret,
        },
        "gates": selected["offline_gates"],
    }
    _write_json(args.offline_output, offline_report)


def _choose_candidate(candidates: list[dict[str, object]]) -> dict[str, object] | None:
    eligible = [candidate for candidate in candidates if candidate.get("eligible") is True]
    if not eligible:
        return None

    def ranking(candidate: dict[str, object]) -> tuple[float, float, float, str]:
        closed_loop = candidate["development_closed_loop"]
        offline = candidate["offline"]
        if not isinstance(closed_loop, dict) or not isinstance(offline, dict):
            raise TypeError("candidate metrics must be dictionaries")
        normalized = offline["mean_normalized_regret"]
        return (
            -float(closed_loop["survival_at_horizon"]),
            -float(closed_loop["mean_pieces_placed"]),
            float(normalized),
            str(candidate["checkpoint_sha256"]),
        )

    return min(eligible, key=ranking)


def _ensure_disjoint(manifest: dict[str, object], evaluation_seeds: list[int]) -> None:
    base = manifest.get("base_seed")
    matches = manifest.get("requested_matches")
    if not isinstance(base, int) or not isinstance(matches, int):
        raise ValueError("dataset manifest lacks a reproducible seed schedule")
    stride = manifest.get("seed_stride", 1)
    if not isinstance(stride, int) or stride <= 0:
        raise ValueError("dataset manifest seed_stride must be positive")
    training_seeds = {base + index * stride for index in range(matches)}
    overlap = training_seeds.intersection(evaluation_seeds)
    if overlap:
        raise ValueError(f"development seeds overlap training data: {sorted(overlap)[:3]}")


def _write_json(path: Path, value: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")


def _sha256_file(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


if __name__ == "__main__":
    main()
