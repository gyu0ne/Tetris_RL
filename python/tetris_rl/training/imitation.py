from __future__ import annotations

import argparse
import copy
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


@dataclass(frozen=True)
class PreparedTrainingData:
    dataset: ValidatedDataset
    stats: FeatureStats


def main() -> None:
    parser = argparse.ArgumentParser(description="Train the v1 placement-level imitation scorer")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--epochs", type=int, default=100, help="maximum epochs")
    parser.add_argument("--min-epochs", type=int, default=20)
    parser.add_argument("--patience", type=int, default=10)
    parser.add_argument("--min-improvement", type=float, default=0.1)
    parser.add_argument("--shuffle-buffer", type=int, default=4_096)
    parser.add_argument("--batch-decisions", type=int, default=64)
    parser.add_argument("--learning-rate", type=float, default=3.0e-4)
    parser.add_argument("--teacher-temperature", type=float, default=1.0)
    parser.add_argument("--teacher-score-scale", type=float, default=1_000.0)
    parser.add_argument("--seed", type=int, default=2026)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--progress-checkpoint", type=Path)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--allow-observed", action="store_true")
    args = parser.parse_args()
    train(args)


def train(args: argparse.Namespace, prepared: PreparedTrainingData | None = None) -> Path:
    if (
        args.epochs <= 0
        or args.min_epochs <= 0
        or args.patience <= 0
        or args.batch_decisions <= 0
        or args.shuffle_buffer <= 0
        or args.threads <= 0
    ):
        raise ValueError("epoch, patience, batch, shuffle and thread values must be positive")
    if args.min_epochs > args.epochs:
        raise ValueError("min-epochs must not exceed epochs")
    if args.min_improvement < 0:
        raise ValueError("min-improvement must be nonnegative")
    if args.teacher_temperature <= 0 or args.teacher_score_scale <= 0:
        raise ValueError("teacher temperature and score scale must be positive")

    torch.manual_seed(args.seed)
    torch.set_num_threads(args.threads)
    torch.use_deterministic_algorithms(True)
    device = torch.device("cpu")

    if prepared is None:
        prepared = prepare_training_data(args.manifest, allow_observed=args.allow_observed)
    dataset = prepared.dataset
    stats = prepared.stats
    model = AfterstateScorer(ModelConfig(input_features=len(FEATURE_NAMES))).to(device)
    optimizer = torch.optim.AdamW(model.parameters(), lr=args.learning_rate, weight_decay=1.0e-4)

    progress_path = _progress_path(args)
    best_validation: EpochMetrics | None = None
    best_state: dict[str, Tensor] | None = None
    best_epoch = 0
    significant_regret: float | None = None
    stale_epochs = 0
    completed_epochs = 0
    history: list[dict[str, object]] = []
    if getattr(args, "resume", False) and progress_path.is_file():
        progress = torch.load(progress_path, map_location=device, weights_only=True)
        _validate_progress(progress, args, dataset)
        model.load_state_dict(progress["model_state"], strict=True)
        optimizer.load_state_dict(progress["optimizer_state"])
        best_state = progress["best_state"]
        best_validation = EpochMetrics(**progress["best_validation"])
        best_epoch = int(progress["best_epoch"])
        significant_regret = progress["significant_regret"]
        stale_epochs = int(progress["stale_epochs"])
        completed_epochs = int(progress["completed_epochs"])
        history = list(progress["training_history"])
        print(
            json.dumps(
                {
                    "event": "training_resumed",
                    "seed": args.seed,
                    "completed_epochs": completed_epochs,
                    "progress_checkpoint": str(progress_path),
                },
                sort_keys=True,
            ),
            flush=True,
        )
    elif progress_path.exists():
        progress_path.unlink()

    for epoch in range(completed_epochs + 1, args.epochs + 1):
        model.train()
        training = _run_epoch(
            model,
            _buffered_shuffle(
                _normalized(dataset, "train", stats),
                args.shuffle_buffer,
                _epoch_seed(args.seed, epoch),
            ),
            args.batch_decisions,
            args.teacher_temperature,
            args.teacher_score_scale,
            optimizer,
            device,
        )
        model.eval()
        with torch.no_grad():
            validation = _run_epoch(
                model,
                _normalized(dataset, "validation", stats),
                args.batch_decisions,
                args.teacher_temperature,
                args.teacher_score_scale,
                None,
                device,
            )
        if validation.decisions == 0:
            raise ValueError("validation split needs at least one decision")

        selected = _is_better(validation, best_validation)
        if selected:
            best_validation = validation
            best_state = copy.deepcopy(model.state_dict())
            best_epoch = epoch

        if (
            significant_regret is None
            or validation.mean_teacher_regret < significant_regret - args.min_improvement
        ):
            significant_regret = validation.mean_teacher_regret
            stale_epochs = 0
        else:
            stale_epochs += 1
        completed_epochs = epoch
        epoch_record = {
            "epoch": epoch,
            "seed": args.seed,
            "train": asdict(training),
            "validation": asdict(validation),
            "selected": selected,
            "stale_epochs": stale_epochs,
        }
        history.append(epoch_record)
        _atomic_torch_save(
            {
                "checkpoint_schema": "afterstate-training-progress-v1",
                "dataset_schema": SCHEMA_VERSION,
                "dataset_id": dataset.manifest["dataset_id"],
                "feature_names": FEATURE_NAMES,
                "training_config": _training_config(args),
                "model_state": model.state_dict(),
                "optimizer_state": optimizer.state_dict(),
                "best_state": best_state,
                "best_validation": asdict(best_validation),
                "best_epoch": best_epoch,
                "significant_regret": significant_regret,
                "stale_epochs": stale_epochs,
                "completed_epochs": completed_epochs,
                "training_history": history,
            },
            progress_path,
        )
        print(
            json.dumps(
                epoch_record,
                sort_keys=True,
            ),
            flush=True,
        )
        if epoch >= args.min_epochs and stale_epochs >= args.patience:
            break

    if best_state is None or best_validation is None:
        raise RuntimeError("training produced no checkpoint candidate")
    model.load_state_dict(best_state, strict=True)
    early_stopped = completed_epochs < args.epochs

    _atomic_torch_save(
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
                "epochs": completed_epochs,
                "max_epochs": args.epochs,
                "min_epochs": args.min_epochs,
                "selected_epoch": best_epoch,
                "early_stopped": early_stopped,
                "patience": args.patience,
                "min_improvement": args.min_improvement,
                "shuffle_buffer": args.shuffle_buffer,
                "batch_decisions": args.batch_decisions,
                "learning_rate": args.learning_rate,
                "teacher_temperature": args.teacher_temperature,
                "teacher_score_scale": args.teacher_score_scale,
                "seed": args.seed,
                "threads": args.threads,
            },
            "validation": asdict(best_validation),
            "training_history": history,
        },
        args.output,
    )
    progress_path.unlink(missing_ok=True)
    print(
        json.dumps(
            {
                "checkpoint": str(args.output),
                "parameters": model.parameter_count(),
                "dataset_id": dataset.manifest["dataset_id"],
                "completed_epochs": completed_epochs,
                "selected_epoch": best_epoch,
                "early_stopped": early_stopped,
            },
            sort_keys=True,
        ),
        flush=True,
    )
    return args.output


def _progress_path(args: argparse.Namespace) -> Path:
    configured = getattr(args, "progress_checkpoint", None)
    if configured is not None:
        return Path(configured)
    return args.output.with_name(f"{args.output.stem}.progress.pt")


def _training_config(args: argparse.Namespace) -> dict[str, object]:
    return {
        "max_epochs": args.epochs,
        "min_epochs": args.min_epochs,
        "patience": args.patience,
        "min_improvement": args.min_improvement,
        "shuffle_buffer": args.shuffle_buffer,
        "batch_decisions": args.batch_decisions,
        "learning_rate": args.learning_rate,
        "teacher_temperature": args.teacher_temperature,
        "teacher_score_scale": args.teacher_score_scale,
        "seed": args.seed,
    }


def _validate_progress(
    progress: dict[str, object],
    args: argparse.Namespace,
    dataset: ValidatedDataset,
) -> None:
    if progress.get("checkpoint_schema") != "afterstate-training-progress-v1":
        raise ValueError("unsupported training progress checkpoint")
    if progress.get("dataset_schema") != SCHEMA_VERSION:
        raise ValueError("training progress dataset schema mismatch")
    if progress.get("dataset_id") != dataset.manifest["dataset_id"]:
        raise ValueError("training progress dataset mismatch")
    if tuple(progress.get("feature_names", ())) != FEATURE_NAMES:
        raise ValueError("training progress feature contract mismatch")
    if progress.get("training_config") != _training_config(args):
        raise ValueError("training progress configuration mismatch")
    completed_epochs = progress.get("completed_epochs")
    history = progress.get("training_history")
    if not isinstance(completed_epochs, int) or completed_epochs < 1:
        raise ValueError("training progress epoch is invalid")
    if not isinstance(history, list) or len(history) != completed_epochs:
        raise ValueError("training progress history is incomplete")
    if progress.get("best_state") is None or progress.get("best_validation") is None:
        raise ValueError("training progress has no best model")


def _atomic_torch_save(payload: dict[str, object], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    torch.save(payload, temporary)
    temporary.replace(path)


def prepare_training_data(manifest: Path, *, allow_observed: bool) -> PreparedTrainingData:
    dataset = validate_dataset(manifest, allow_observed=allow_observed)
    return PreparedTrainingData(dataset, compute_feature_stats(dataset))


def _normalized(dataset: ValidatedDataset, split: Split, stats: FeatureStats) -> Iterator[Decision]:
    for decision in iter_decisions(dataset, split):
        yield normalize(decision, stats)


def _buffered_shuffle(
    decisions: Iterable[Decision], buffer_size: int, seed: int
) -> Iterator[Decision]:
    """Deterministic bounded-memory shuffle for a streamed gzip dataset."""

    randomizer = random.Random(seed)
    buffer: list[Decision] = []
    for decision in decisions:
        if len(buffer) < buffer_size:
            buffer.append(decision)
            continue
        selected = randomizer.randrange(buffer_size)
        yield buffer[selected]
        buffer[selected] = decision
    randomizer.shuffle(buffer)
    yield from buffer


def _epoch_seed(training_seed: int, epoch: int) -> int:
    return (training_seed << 32) ^ epoch


def _is_better(current: EpochMetrics, best: EpochMetrics | None) -> bool:
    if best is None:
        return True
    return (current.mean_teacher_regret, current.loss) < (
        best.mean_teacher_regret,
        best.loss,
    )


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
