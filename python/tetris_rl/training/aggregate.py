from __future__ import annotations

import argparse
import gzip
import importlib
import json
from hashlib import sha256
from pathlib import Path

import numpy as np
import torch

from tetris_rl.features import FEATURE_NAMES
from tetris_rl.models import load_scorer
from tetris_rl.training.dataset import validate_dataset


def main() -> None:
    parser = argparse.ArgumentParser(description="Add teacher labels on learner-visited states")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--records", type=Path, required=True)
    parser.add_argument("--output-manifest", type=Path, required=True)
    parser.add_argument("--matches", type=int, default=1_024)
    parser.add_argument("--decisions-per-match", type=int, default=250)
    parser.add_argument("--target-decisions", type=int, default=250_000)
    parser.add_argument("--parallel-games", type=int, default=128)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--allow-observed", action="store_true")
    args = parser.parse_args()
    if (
        min(
            args.matches,
            args.decisions_per_match,
            args.target_decisions,
            args.parallel_games,
            args.threads,
        )
        <= 0
    ):
        raise ValueError("aggregation counts and threads must be positive")
    if args.target_decisions > args.matches * args.decisions_per_match:
        raise ValueError("target-decisions exceeds the requested trajectory capacity")

    torch.set_num_threads(args.threads)
    dataset = validate_dataset(args.manifest, allow_observed=args.allow_observed)
    scorer = load_scorer(args.checkpoint, allow_observed=args.allow_observed)
    if scorer.metadata.get("dataset_id") != dataset.manifest.get("dataset_id"):
        raise ValueError("checkpoint and parent dataset_id differ")
    if args.records.resolve() == dataset.records_path.resolve():
        raise ValueError("aggregation output must not overwrite its parent records")
    if args.records.parent.resolve() != args.output_manifest.parent.resolve():
        raise ValueError("records and output-manifest must share one dataset directory")

    base_seed = int(dataset.manifest["base_seed"])
    seed_stride = int(dataset.manifest["seed_stride"])
    parent_matches = int(dataset.manifest["requested_matches"])
    seeds = [base_seed + (parent_matches + index) * seed_stride for index in range(args.matches)]
    args.records.parent.mkdir(parents=True, exist_ok=True)
    args.output_manifest.parent.mkdir(parents=True, exist_ok=True)

    added = 0
    added_min_candidates: int | None = None
    added_max_candidates = 0
    with (
        args.records.open("wb") as raw_output,
        gzip.GzipFile(
            filename="", mode="wb", fileobj=raw_output, mtime=0, compresslevel=6
        ) as output,
    ):
        with gzip.open(dataset.records_path, "rb") as parent:
            while block := parent.read(1024 * 1024):
                output.write(block)

        for start in range(0, len(seeds), args.parallel_games):
            batch_seeds = seeds[start : start + args.parallel_games]
            bridge = _new_bridge(batch_seeds)
            for ply in range(args.decisions_per_match):
                raw_features, raw_scores, offsets, done = bridge.labeled_candidates()
                candidate_count = offsets[-1]
                features = np.frombuffer(raw_features, dtype="<i4").reshape(
                    candidate_count, len(FEATURE_NAMES)
                )
                teacher_scores = np.frombuffer(raw_scores, dtype="<i8")
                if teacher_scores.size != candidate_count:
                    raise ValueError("engine feature and teacher-score counts differ")
                logits = scorer.score(torch.from_numpy(features.copy()).to(dtype=torch.float32))
                selections = [-1] * len(batch_seeds)
                for game, is_done in enumerate(done):
                    begin = offsets[game]
                    end = offsets[game + 1]
                    if is_done:
                        continue
                    selections[game] = int(torch.argmax(logits[begin:end]).item())
                    if added >= args.target_decisions:
                        continue
                    scores = teacher_scores[begin:end].tolist()
                    order = sorted(range(len(scores)), key=lambda index: (-scores[index], index))
                    ranks = [0] * len(scores)
                    for rank, index in enumerate(order):
                        ranks[index] = rank
                    chosen = order[0]
                    margin = scores[chosen] - scores[order[1]] if len(order) > 1 else 0
                    record = {
                        "schema_version": dataset.manifest["schema_version"],
                        "rules_hash": dataset.manifest["rules_hash"],
                        "engine_revision": dataset.manifest["engine_revision"],
                        "mechanics_status": dataset.manifest["mechanics_status"],
                        "action_space": dataset.manifest["action_space"],
                        "record_kind": "learner-state-teacher-label-v1",
                        "match_id": f"aggregate-{batch_seeds[game]:016x}",
                        "seed": batch_seeds[game],
                        "ply": ply,
                        "candidates": [
                            {
                                "features": features[begin + index].tolist(),
                                "teacher_score": score,
                                "rank": ranks[index],
                            }
                            for index, score in enumerate(scores)
                        ],
                        "chosen_index": chosen,
                        "top_two_margin": margin,
                    }
                    output.write(json.dumps(record, separators=(",", ":")).encode() + b"\n")
                    count = end - begin
                    added_min_candidates = (
                        count if added_min_candidates is None else min(added_min_candidates, count)
                    )
                    added_max_candidates = max(added_max_candidates, count)
                    added += 1
                bridge.step(selections)
                if added >= args.target_decisions:
                    break
            if added >= args.target_decisions:
                break

    if added < args.target_decisions:
        raise RuntimeError(
            f"learner trajectories produced {added} decisions, expected {args.target_decisions}"
        )
    digest = _sha256_file(args.records)
    manifest = dict(dataset.manifest)
    history = list(manifest.get("aggregation_history", []))
    history.append(
        {
            "schema_version": "learner-state-aggregation-v1",
            "parent_dataset_id": dataset.manifest["dataset_id"],
            "checkpoint_sha256": _sha256_file(args.checkpoint),
            "added_decisions": added,
            "added_matches": args.matches,
            "decisions_per_match": args.decisions_per_match,
        }
    )
    manifest.update(
        {
            "dataset_id": digest,
            "records_sha256": digest,
            "records_file": args.records.name,
            "requested_matches": parent_matches + args.matches,
            "completed_matches": int(dataset.manifest["completed_matches"]) + args.matches,
            "decisions": int(dataset.manifest["decisions"]) + added,
            "min_candidates": min(
                int(dataset.manifest["min_candidates"]), int(added_min_candidates or 0)
            ),
            "max_candidates": max(int(dataset.manifest["max_candidates"]), added_max_candidates),
            "aggregation_history": history,
        }
    )
    args.output_manifest.write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(manifest, sort_keys=True))


def _new_bridge(seeds: list[int]):  # type: ignore[no-untyped-def]
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
