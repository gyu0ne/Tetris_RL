from __future__ import annotations

import gzip
import json
from collections.abc import Iterator
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
from typing import Literal

from tetris_rl.features import ACTION_SPACE_ID, FEATURE_NAMES, MECHANICS_STATUS, SCHEMA_VERSION

Split = Literal["train", "validation", "all"]


class DatasetValidationError(ValueError):
    pass


@dataclass(frozen=True)
class Decision:
    match_id: str
    seed: int
    features: tuple[tuple[float, ...], ...]
    teacher_scores: tuple[float, ...]
    chosen_index: int


@dataclass(frozen=True)
class FeatureStats:
    mean: tuple[float, ...]
    std: tuple[float, ...]
    candidates: int


@dataclass(frozen=True)
class ValidatedDataset:
    manifest_path: Path
    records_path: Path
    manifest: dict[str, object]


def validate_dataset(manifest_path: Path, *, allow_observed: bool) -> ValidatedDataset:
    manifest_path = manifest_path.resolve()
    with manifest_path.open("r", encoding="utf-8") as source:
        manifest = json.load(source)
    _expect(manifest.get("schema_version") == SCHEMA_VERSION, "unsupported schema_version")
    _expect(manifest.get("action_space") == ACTION_SPACE_ID, "unsupported action_space")
    _expect(tuple(manifest.get("feature_names", ())) == FEATURE_NAMES, "feature contract mismatch")
    mechanics_status = manifest.get("mechanics_status")
    if mechanics_status == MECHANICS_STATUS:
        _expect(allow_observed, "OBSERVED dataset requires --allow-observed")
    else:
        _expect(mechanics_status == "CONFORMANT", "unknown mechanics_status")

    records_name = manifest.get("records_file")
    _expect(
        isinstance(records_name, str) and Path(records_name).name == records_name,
        "invalid records_file",
    )
    records_path = manifest_path.parent / records_name
    _expect(records_path.is_file(), f"records file does not exist: {records_path}")
    actual_hash = _sha256_file(records_path)
    _expect(actual_hash == manifest.get("records_sha256"), "records SHA-256 mismatch")
    _expect(actual_hash == manifest.get("dataset_id"), "dataset_id mismatch")
    return ValidatedDataset(manifest_path, records_path, manifest)


def iter_decisions(dataset: ValidatedDataset, split: Split) -> Iterator[Decision]:
    with gzip.open(dataset.records_path, "rt", encoding="utf-8") as source:
        for line_number, line in enumerate(source, 1):
            if not line.strip():
                continue
            try:
                record = json.loads(line)
                decision = _parse_decision(record, dataset.manifest)
            except (KeyError, TypeError, ValueError) as error:
                raise DatasetValidationError(
                    f"invalid record at line {line_number}: {error}"
                ) from error
            is_validation = decision.seed % 5 == 0
            if split == "all" or (split == "validation") == is_validation:
                yield decision


def compute_feature_stats(dataset: ValidatedDataset) -> FeatureStats:
    count = 0
    mean = [0.0] * len(FEATURE_NAMES)
    squared_delta = [0.0] * len(FEATURE_NAMES)
    for decision in iter_decisions(dataset, "train"):
        for candidate in decision.features:
            count += 1
            for index, value in enumerate(candidate):
                delta = value - mean[index]
                mean[index] += delta / count
                squared_delta[index] += delta * (value - mean[index])
    _expect(count > 1, "training split needs at least two candidates")
    std = [max((value / (count - 1)) ** 0.5, 1.0e-6) for value in squared_delta]
    return FeatureStats(tuple(mean), tuple(std), count)


def normalize(decision: Decision, stats: FeatureStats) -> Decision:
    features = tuple(
        tuple(
            (value - stats.mean[index]) / stats.std[index] for index, value in enumerate(candidate)
        )
        for candidate in decision.features
    )
    return Decision(
        decision.match_id,
        decision.seed,
        features,
        decision.teacher_scores,
        decision.chosen_index,
    )


def _parse_decision(record: dict[str, object], manifest: dict[str, object]) -> Decision:
    _expect(record["schema_version"] == manifest["schema_version"], "record schema mismatch")
    _expect(record["rules_hash"] == manifest["rules_hash"], "record rules hash mismatch")
    _expect(record["engine_revision"] == manifest["engine_revision"], "record revision mismatch")
    _expect(record["mechanics_status"] == manifest["mechanics_status"], "record status mismatch")
    _expect(record["action_space"] == manifest["action_space"], "record action space mismatch")
    candidates = record["candidates"]
    _expect(isinstance(candidates, list) and candidates, "decision has no candidates")
    chosen_index = int(record["chosen_index"])
    _expect(0 <= chosen_index < len(candidates), "chosen_index out of range")

    parsed_features: list[tuple[float, ...]] = []
    scores: list[float] = []
    ranks: list[int] = []
    for candidate in candidates:
        _expect(isinstance(candidate, dict), "candidate must be an object")
        values = tuple(float(value) for value in candidate["features"])
        _expect(len(values) == len(FEATURE_NAMES), "candidate feature count mismatch")
        parsed_features.append(values)
        scores.append(float(candidate["teacher_score"]))
        ranks.append(int(candidate["rank"]))
    _expect(sorted(ranks) == list(range(len(candidates))), "candidate ranks are not a permutation")
    _expect(ranks[chosen_index] == 0, "chosen candidate is not rank zero")
    return Decision(
        match_id=str(record["match_id"]),
        seed=int(record["seed"]),
        features=tuple(parsed_features),
        teacher_scores=tuple(scores),
        chosen_index=chosen_index,
    )


def _sha256_file(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def _expect(condition: bool, message: str) -> None:
    if not condition:
        raise DatasetValidationError(message)
