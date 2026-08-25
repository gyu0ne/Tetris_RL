from __future__ import annotations

import argparse
import json
from hashlib import sha256
from pathlib import Path

import torch

from tetris_rl.models import load_scorer


def main() -> None:
    parser = argparse.ArgumentParser(description="Promote a checkpoint with passing evaluations")
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--offline-report", type=Path, required=True)
    parser.add_argument("--closed-loop-report", type=Path, required=True)
    parser.add_argument("--selection-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--allow-observed", action="store_true")
    args = parser.parse_args()

    source_hash = _sha256_file(args.checkpoint)
    offline = _read_report(args.offline_report, "offline-imitation-evaluation-v1")
    closed_loop = _read_report(args.closed_loop_report, "closed-loop-solo-evaluation-v1")
    selection = _read_report(args.selection_report, "imitation-candidate-selection-v1")
    _expect(offline["checkpoint_sha256"] == source_hash, "offline checkpoint SHA-256")
    _expect(closed_loop["checkpoint_sha256"] == source_hash, "closed-loop checkpoint SHA-256")
    _expect(bool(offline["gates"]["passed"]), "offline gates did not pass")
    _expect(bool(closed_loop["gates"]["passed"]), "closed-loop gates did not pass")
    _expect(bool(selection["passed"]), "candidate selection did not pass")
    _expect(selection["selected_checkpoint_sha256"] == source_hash, "selection checkpoint SHA-256")

    loaded = load_scorer(args.checkpoint, allow_observed=args.allow_observed)
    _expect(offline["dataset_id"] == loaded.metadata["dataset_id"], "offline dataset ID")
    _expect(closed_loop["dataset_id"] == loaded.metadata["dataset_id"], "closed-loop dataset ID")
    _expect(selection["dataset_id"] == loaded.metadata["dataset_id"], "selection dataset ID")
    _expect(
        offline["engine_revision"] == loaded.metadata["engine_revision"],
        "offline engine revision",
    )
    _expect(
        closed_loop["engine_revision"] == loaded.metadata["engine_revision"],
        "closed-loop engine revision",
    )
    _expect(
        selection["engine_revision"] == loaded.metadata["engine_revision"],
        "selection engine revision",
    )
    payload = torch.load(args.checkpoint, map_location="cpu", weights_only=True)
    payload["promotion"] = {
        "schema_version": "solo-imitation-promotion-v2",
        "source_checkpoint_sha256": source_hash,
        "selection": selection,
        "offline": offline,
        "closed_loop": closed_loop,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    torch.save(payload, args.output)
    promoted = load_scorer(args.output, allow_observed=args.allow_observed)
    print(
        json.dumps(
            {
                "checkpoint": str(args.output),
                "checkpoint_sha256": _sha256_file(args.output),
                "parameters": promoted.model.parameter_count(),
                "promotion_schema": promoted.metadata["promotion"]["schema_version"],
            },
            sort_keys=True,
        )
    )


def _read_report(path: Path, schema: str) -> dict[str, object]:
    with path.open("r", encoding="utf-8") as source:
        report = json.load(source)
    _expect(report.get("schema_version") == schema, f"report schema {path}")
    return report


def _sha256_file(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def _expect(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(f"checkpoint promotion failed: {message}")


if __name__ == "__main__":
    main()
