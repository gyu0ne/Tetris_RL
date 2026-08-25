from __future__ import annotations

import argparse
import json
from collections.abc import Iterable, Iterator
from dataclasses import asdict, dataclass
from hashlib import sha256
from itertools import islice
from pathlib import Path
from typing import Protocol

import torch
from torch import Tensor

from tetris_rl.models import load_scorer
from tetris_rl.training.dataset import Decision, iter_decisions, validate_dataset


class CandidateScorer(Protocol):
    def score(self, features: Tensor) -> Tensor: ...


@dataclass(frozen=True)
class OfflineMetrics:
    decisions: int
    positive_margin_decisions: int
    tie_aware_optimal_rate: float
    positive_margin_agreement: float
    mean_teacher_regret: float
    mean_positive_margin: float | None
    mean_normalized_regret: float | None


def main() -> None:
    parser = argparse.ArgumentParser(description="Evaluate v1 scorer on held-out records")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--split", choices=("train", "validation", "all"), default="validation")
    parser.add_argument("--batch-decisions", type=int, default=64)
    parser.add_argument("--max-decisions", type=int)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--allow-observed", action="store_true")
    parser.add_argument("--require-gates", action="store_true")
    parser.add_argument("--min-tie-aware", type=float, default=0.97)
    parser.add_argument("--min-positive-margin", type=float, default=0.95)
    parser.add_argument("--max-normalized-regret", type=float, default=0.05)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.batch_decisions <= 0 or args.threads <= 0:
        raise ValueError("batch-decisions and threads must be positive")
    if args.max_decisions is not None and args.max_decisions <= 0:
        raise ValueError("max-decisions must be positive")

    torch.set_num_threads(args.threads)
    dataset = validate_dataset(args.manifest, allow_observed=args.allow_observed)
    scorer = load_scorer(args.checkpoint, allow_observed=args.allow_observed)
    if scorer.metadata.get("dataset_id") != dataset.manifest.get("dataset_id"):
        raise ValueError("checkpoint and manifest dataset_id differ")

    decisions: Iterable[Decision] = iter_decisions(dataset, args.split)
    if args.max_decisions is not None:
        decisions = islice(decisions, args.max_decisions)
    metrics = evaluate_decisions(scorer, decisions, args.batch_decisions)
    gates = {
        "tie_aware_optimal_rate": metrics.tie_aware_optimal_rate >= args.min_tie_aware,
        "positive_margin_agreement": (
            metrics.positive_margin_agreement >= args.min_positive_margin
        ),
        "mean_normalized_regret": (
            metrics.mean_normalized_regret is not None
            and metrics.mean_normalized_regret <= args.max_normalized_regret
        ),
    }
    report = {
        "schema_version": "offline-imitation-evaluation-v1",
        "checkpoint": str(args.checkpoint),
        "checkpoint_sha256": _sha256_file(args.checkpoint),
        "dataset_id": dataset.manifest["dataset_id"],
        "engine_revision": scorer.metadata["engine_revision"],
        "split": args.split,
        "metrics": asdict(metrics),
        "thresholds": {
            "min_tie_aware": args.min_tie_aware,
            "min_positive_margin": args.min_positive_margin,
            "max_normalized_regret": args.max_normalized_regret,
        },
        "gates": {**gates, "passed": all(gates.values())},
    }
    serialized = json.dumps(report, sort_keys=True)
    print(serialized)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(serialized + "\n", encoding="utf-8")
    if args.require_gates and not report["gates"]["passed"]:
        raise SystemExit(3)


def evaluate_decisions(
    scorer: CandidateScorer,
    decisions: Iterable[Decision],
    batch_decisions: int,
) -> OfflineMetrics:
    total = 0
    optimal = 0
    positive_total = 0
    positive_correct = 0
    total_regret = 0.0
    positive_margin_sum = 0.0

    for batch in _batches(decisions, batch_decisions):
        flat_features = [candidate for decision in batch for candidate in decision.features]
        features = torch.tensor(flat_features, dtype=torch.float32)
        logits = scorer.score(features)
        offset = 0
        for decision in batch:
            size = len(decision.features)
            predicted = int(torch.argmax(logits[offset : offset + size]).item())
            teacher_best = max(decision.teacher_scores)
            predicted_teacher = decision.teacher_scores[predicted]
            best_count = sum(score == teacher_best for score in decision.teacher_scores)
            regret = teacher_best - predicted_teacher

            total += 1
            optimal += int(regret == 0)
            total_regret += regret
            if best_count == 1:
                positive_total += 1
                positive_correct += int(regret == 0)
                runner_up = max(
                    score
                    for index, score in enumerate(decision.teacher_scores)
                    if index != decision.chosen_index
                )
                positive_margin_sum += teacher_best - runner_up
            offset += size

    if total == 0:
        raise ValueError("offline evaluation needs at least one decision")
    mean_regret = total_regret / total
    mean_positive_margin = positive_margin_sum / positive_total if positive_total > 0 else None
    normalized_regret = (
        mean_regret / mean_positive_margin if mean_positive_margin is not None else None
    )
    return OfflineMetrics(
        decisions=total,
        positive_margin_decisions=positive_total,
        tie_aware_optimal_rate=optimal / total,
        positive_margin_agreement=(positive_correct / positive_total if positive_total else 1.0),
        mean_teacher_regret=mean_regret,
        mean_positive_margin=mean_positive_margin,
        mean_normalized_regret=normalized_regret,
    )


def _batches(decisions: Iterable[Decision], size: int) -> Iterator[list[Decision]]:
    iterator = iter(decisions)
    while batch := list(islice(iterator, size)):
        yield batch


def _sha256_file(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


if __name__ == "__main__":
    main()
