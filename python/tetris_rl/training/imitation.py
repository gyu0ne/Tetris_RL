from __future__ import annotations

import argparse
import json
import random
from collections.abc import Iterable, Iterator
from dataclasses import asdict, dataclass
from pathlib import Path

import torch
from torch import Tensor

from tetris_rl.features import FEATURE_NAMES, SCHEMA_VERSION
from tetris_rl.models import AfterstateScorer, ModelConfig
from tetris_rl.training.dataset import (
    Decision,
    FeatureStats,
    Split,
    ValidatedDataset,
    compute_feature_stats,
    iter_decisions,
    normalize,
    validate_dataset,
)


@dataclass(frozen=True)
class EpochMetrics:
    decisions: int
    loss: float
    top_one_accuracy: float
    mean_teacher_regret: float


def main() -> None:
    parser = argparse.ArgumentParser(description="Train the v1 placement-level imitation scorer")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--epochs", type=int, default=8)
    parser.add_argument("--batch-decisions", type=int, default=64)
    parser.add_argument("--learning-rate", type=float, default=3.0e-4)
    parser.add_argument("--teacher-temperature", type=float, default=1.0)
    parser.add_argument("--teacher-score-scale", type=float, default=1_000.0)
    parser.add_argument("--seed", type=int, default=2026)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--allow-observed", action="store_true")
    args = parser.parse_args()
    train(args)


def train(args: argparse.Namespace) -> None:
    if args.epochs <= 0 or args.batch_decisions <= 0 or args.threads <= 0:
        raise ValueError("epochs, batch-decisions and threads must be positive")
    if args.teacher_temperature <= 0 or args.teacher_score_scale <= 0:
        raise ValueError("teacher temperature and score scale must be positive")

    random.seed(args.seed)
    torch.manual_seed(args.seed)
    torch.set_num_threads(args.threads)
    torch.use_deterministic_algorithms(True)
    device = torch.device("cpu")

    dataset = validate_dataset(args.manifest, allow_observed=args.allow_observed)
    stats = compute_feature_stats(dataset)
    model = AfterstateScorer(ModelConfig(input_features=len(FEATURE_NAMES))).to(device)
    optimizer = torch.optim.AdamW(model.parameters(), lr=args.learning_rate, weight_decay=1.0e-4)

    last_validation: EpochMetrics | None = None
    for epoch in range(1, args.epochs + 1):
        model.train()
        training = _run_epoch(
            model,
            _normalized(dataset, "train", stats),
            args.batch_decisions,
            args.teacher_temperature,
            args.teacher_score_scale,
            optimizer,
            device,
        )
        model.eval()
        with torch.no_grad():
            last_validation = _run_epoch(
                model,
                _normalized(dataset, "validation", stats),
                args.batch_decisions,
                args.teacher_temperature,
                args.teacher_score_scale,
                None,
                device,
            )
        print(
            json.dumps(
                {
                    "epoch": epoch,
                    "train": asdict(training),
                    "validation": asdict(last_validation),
                },
                sort_keys=True,
            )
        )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    torch.save(
        {
            "checkpoint_schema": "afterstate-scorer-v1",
            "dataset_schema": SCHEMA_VERSION,
            "dataset_id": dataset.manifest["dataset_id"],
            "dataset_manifest": dataset.manifest,
            "engine_revision": dataset.manifest["engine_revision"],
            "mechanics_status": dataset.manifest["mechanics_status"],
            "feature_names": FEATURE_NAMES,
            "feature_mean": stats.mean,
            "feature_std": stats.std,
            "model_config": model.config.to_dict(),
            "model_state": model.state_dict(),
            "training": {
                "epochs": args.epochs,
                "batch_decisions": args.batch_decisions,
                "learning_rate": args.learning_rate,
                "teacher_temperature": args.teacher_temperature,
                "teacher_score_scale": args.teacher_score_scale,
                "seed": args.seed,
                "threads": args.threads,
            },
            "validation": asdict(last_validation) if last_validation else None,
        },
        args.output,
    )
    print(
        json.dumps(
            {
                "checkpoint": str(args.output),
                "parameters": model.parameter_count(),
                "dataset_id": dataset.manifest["dataset_id"],
            },
            sort_keys=True,
        )
    )


def _normalized(dataset: ValidatedDataset, split: Split, stats: FeatureStats) -> Iterator[Decision]:
    for decision in iter_decisions(dataset, split):
        yield normalize(decision, stats)


def _run_epoch(
    model: AfterstateScorer,
    decisions: Iterable[Decision],
    batch_decisions: int,
    teacher_temperature: float,
    teacher_score_scale: float,
    optimizer: torch.optim.Optimizer | None,
    device: torch.device,
) -> EpochMetrics:
    total_loss = 0.0
    total_correct = 0
    total_regret = 0.0
    total_decisions = 0
    for batch in _batches(decisions, batch_decisions):
        if optimizer is not None:
            optimizer.zero_grad(set_to_none=True)
        loss, correct, regret = _batch_loss(
            model, batch, teacher_temperature, teacher_score_scale, device
        )
        if optimizer is not None:
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 5.0)
            optimizer.step()
        batch_size = len(batch)
        total_loss += float(loss.detach()) * batch_size
        total_correct += correct
        total_regret += regret
        total_decisions += batch_size
    if total_decisions == 0:
        return EpochMetrics(0, 0.0, 0.0, 0.0)
    return EpochMetrics(
        total_decisions,
        total_loss / total_decisions,
        total_correct / total_decisions,
        total_regret / total_decisions,
    )


def _batch_loss(
    model: AfterstateScorer,
    batch: list[Decision],
    teacher_temperature: float,
    teacher_score_scale: float,
    device: torch.device,
) -> tuple[Tensor, int, float]:
    flat_features = [candidate for decision in batch for candidate in decision.features]
    features = torch.tensor(flat_features, dtype=torch.float32, device=device)
    logits = model(features)
    losses: list[Tensor] = []
    correct = 0
    regret = 0.0
    offset = 0
    for decision in batch:
        size = len(decision.features)
        decision_logits = logits[offset : offset + size]
        teacher_scores = torch.tensor(decision.teacher_scores, dtype=torch.float32, device=device)
        teacher_target = torch.softmax(
            teacher_scores / teacher_score_scale / teacher_temperature, dim=0
        )
        losses.append(-(teacher_target * torch.log_softmax(decision_logits, dim=0)).sum())
        predicted = int(torch.argmax(decision_logits).item())
        correct += int(predicted == decision.chosen_index)
        regret += (
            decision.teacher_scores[decision.chosen_index] - decision.teacher_scores[predicted]
        )
        offset += size
    return torch.stack(losses).mean(), correct, regret


def _batches(decisions: Iterable[Decision], batch_size: int) -> Iterator[list[Decision]]:
    batch: list[Decision] = []
    for decision in decisions:
        batch.append(decision)
        if len(batch) == batch_size:
            yield batch
            batch = []
    if batch:
        yield batch


if __name__ == "__main__":
    main()
