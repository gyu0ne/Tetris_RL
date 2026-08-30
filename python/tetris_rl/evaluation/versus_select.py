import argparse
import hashlib
import json
import shutil
from dataclasses import asdict
from pathlib import Path

import torch

from tetris_rl.evaluation.versus import MatchSummary, evaluate_side
from tetris_rl.models import LoadedVersusActor, load_versus_actor

REPORT_SCHEMA = "versus-champion-selection-v1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Select a robust versus checkpoint")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--anchor", type=Path, action="append", default=[])
    parser.add_argument("--report", type=Path)
    parser.add_argument("--selected", type=Path)
    parser.add_argument("--shortlist", type=int, default=6)
    parser.add_argument("--seeds", type=int, default=8)
    parser.add_argument("--horizon", type=int, default=2_000)
    parser.add_argument("--cadences", default="8,12,15")
    parser.add_argument("--base-seed", type=int, default=970_001)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--allow-observed", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    cadences = [int(value) for value in args.cadences.split(",")]
    if (
        args.shortlist <= 0
        or args.seeds <= 0
        or args.horizon <= 0
        or args.threads <= 0
        or not cadences
        or any(cadence <= 0 for cadence in cadences)
    ):
        raise ValueError("selection counts, horizon, threads and cadences must be positive")
    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    candidates = _discover_candidates(args.output_dir)
    shortlist = _shortlist_candidates(candidates, args.shortlist)
    anchors = list(dict.fromkeys(str(path) for path in args.anchor))
    reference = args.output_dir / "reference-model.pt"
    if reference.is_file():
        anchors.append(str(reference))
    anchors = list(dict.fromkeys(anchors))
    for path in [*shortlist, *anchors]:
        if not Path(path).is_file():
            raise FileNotFoundError(f"selection checkpoint not found: {path}")
    opponents = list(dict.fromkeys([*anchors, *shortlist]))
    if not any(opponent != shortlist[0] for opponent in opponents):
        raise ValueError("champion selection requires at least one distinct opponent")

    cache: dict[str, LoadedVersusActor] = {}
    seeds = [args.base_seed + 104_729 * index for index in range(args.seeds)]
    evaluations = []
    summaries = []
    for candidate_path in shortlist:
        candidate = _load(cache, candidate_path, args.allow_observed)
        candidate_rows = []
        for opponent_path in opponents:
            if opponent_path == candidate_path:
                continue
            opponent = _load(cache, opponent_path, args.allow_observed)
            for cadence in cadences:
                combined = _evaluate_pair(
                    candidate,
                    opponent,
                    seeds,
                    args.horizon,
                    cadence,
                )
                row = {
                    "candidate": candidate_path,
                    "opponent": opponent_path,
                    "frames_per_placement": cadence,
                    **asdict(combined),
                    "score": combined.score,
                    "completion_rate": combined.completion_rate,
                }
                evaluations.append(row)
                candidate_rows.append(row)
                print(json.dumps({"event": "candidate_matchup", **row}, sort_keys=True))
        summary = _candidate_summary(candidate_path, candidate_rows)
        summaries.append(summary)
        print(json.dumps({"event": "candidate_summary", **summary}, sort_keys=True))

    selected_summary = max(
        summaries,
        key=lambda row: (
            row["robust_score"],
            row["mean_score"],
            row["completion_rate"],
            row["outgoing_attack_per_piece"],
            -row["danger_rate"],
        ),
    )
    selected_source = Path(str(selected_summary["checkpoint"]))
    selected_path = args.selected or args.output_dir / "selected-model.pt"
    report_path = args.report or args.output_dir / "selection-report.json"
    selected_path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(selected_source, selected_path)
    report = {
        "schema_version": REPORT_SCHEMA,
        "base_seed": args.base_seed,
        "seeds": args.seeds,
        "horizon": args.horizon,
        "cadences": cadences,
        "shortlist": shortlist,
        "anchors": anchors,
        "summaries": summaries,
        "evaluations": evaluations,
        "selected_source": str(selected_source),
        "selected_checkpoint": str(selected_path),
        "selected_sha256": _sha256_file(selected_path),
    }
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"event": "champion_selected", **report}, sort_keys=True))


def _discover_candidates(output_dir: Path) -> list[str]:
    candidates = sorted((output_dir / "snapshots").glob("update-*-model.pt"))
    if candidates:
        metadata = torch.load(candidates[0], map_location="cpu", weights_only=True)
        config = metadata.get("training_config", metadata.get("config", {}))
        interval = int(config.get("pool_promotion_interval_updates", 1))
        candidates = [path for path in candidates if int(path.stem.split("-")[1]) % interval == 0]
    final = output_dir / "model.pt"
    if final.is_file():
        candidates.append(final)
    if not candidates:
        raise FileNotFoundError(f"no versus candidates found under {output_dir}")
    return [str(path) for path in candidates]


def _shortlist_candidates(candidates: list[str], limit: int) -> list[str]:
    if len(candidates) <= limit:
        return candidates
    ranked = sorted(candidates, key=_training_robustness, reverse=True)
    selected = ranked[: max(limit - 2, 1)]
    selected.append(candidates[-1])
    selected.append(candidates[len(candidates) // 2])
    return list(dict.fromkeys(selected))[:limit]


def _training_robustness(checkpoint: str) -> tuple[float, float, float]:
    checkpoint_path = Path(checkpoint)
    progress = (
        checkpoint_path.parent / "latest.pt"
        if checkpoint_path.name == "model.pt"
        else Path(checkpoint.replace("-model.pt", ".pt"))
    )
    if not progress.is_file():
        return 0.0, 0.0, 0.0
    payload = torch.load(progress, map_location="cpu", weights_only=True)
    history = payload.get("history", [])
    scores = []
    for prefix in ("bootstrap", "historical"):
        wins = sum(int(row.get(f"{prefix}_wins", 0)) for row in history)
        losses = sum(int(row.get(f"{prefix}_losses", 0)) for row in history)
        draws = sum(int(row.get(f"{prefix}_draws", 0)) for row in history)
        games = wins + losses + draws
        scores.append((wins + 0.5 * draws) / games if games else 0.5)
    attack = sum(float(row.get("outgoing_attack_per_piece", 0.0)) for row in history)
    attack /= len(history) if history else 1
    return min(scores), sum(scores) / len(scores), attack


def _load(
    cache: dict[str, LoadedVersusActor], checkpoint: str, allow_observed: bool
) -> LoadedVersusActor:
    if checkpoint not in cache:
        cache[checkpoint] = load_versus_actor(Path(checkpoint), allow_observed=allow_observed)
    return cache[checkpoint]


def _evaluate_pair(
    candidate: LoadedVersusActor,
    opponent: LoadedVersusActor,
    seeds: list[int],
    horizon: int,
    cadence: int,
) -> MatchSummary:
    left = evaluate_side(candidate, opponent, seeds, horizon, cadence, candidate_side=0)
    right = evaluate_side(candidate, opponent, seeds, horizon, cadence, candidate_side=1)
    return MatchSummary(
        **{field: getattr(left, field) + getattr(right, field) for field in asdict(left)}
    )


def _candidate_summary(
    checkpoint: str, rows: list[dict[str, object]]
) -> dict[str, str | float | int]:
    opponent_scores: dict[str, list[float]] = {}
    totals = {field: 0 for field in asdict(MatchSummary(0, 0, 0, 0))}
    for row in rows:
        opponent_scores.setdefault(str(row["opponent"]), []).append(float(row["score"]))
        for field in totals:
            totals[field] += int(row[field])
    combined = MatchSummary(**totals)
    per_opponent = [sum(values) / len(values) for values in opponent_scores.values()]
    pieces = combined.pieces
    return {
        "checkpoint": checkpoint,
        "matchups": len(rows),
        "robust_score": min(per_opponent),
        "mean_score": combined.score,
        "completion_rate": combined.completion_rate,
        "outgoing_attack_per_piece": combined.outgoing_attack / pieces if pieces else 0.0,
        "danger_rate": combined.danger_decisions / pieces if pieces else 0.0,
    }


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


if __name__ == "__main__":
    main()
