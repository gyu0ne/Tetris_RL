import argparse
import hashlib
import json
import shutil
from dataclasses import asdict, dataclass
from pathlib import Path

import torch

from tetris_rl.evaluation.versus import MatchSummary, evaluate_side
from tetris_rl.models import LoadedVersusActor, load_versus_actor

REPORT_SCHEMA = "versus-champion-selection-v2"


@dataclass(frozen=True)
class PromotionThresholds:
    min_score_delta: float = -0.03
    min_direct_baseline_score: float = 0.47
    min_attack_ratio: float = 1.20
    max_danger_ratio: float = 1.15
    max_holes_ratio: float = 1.15


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Select a robust versus checkpoint")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--anchor", type=Path, action="append", default=[])
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--selected", type=Path)
    parser.add_argument("--shortlist", type=int, default=6)
    parser.add_argument("--seeds", type=int, default=8)
    parser.add_argument("--horizon", type=int, default=2_000)
    parser.add_argument("--cadences", default="8,12,15")
    parser.add_argument("--base-seed", type=int, default=970_001)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--min-score-delta", type=float, default=-0.03)
    parser.add_argument("--min-direct-baseline-score", type=float, default=0.47)
    parser.add_argument("--min-attack-ratio", type=float, default=1.20)
    parser.add_argument("--max-danger-ratio", type=float, default=1.15)
    parser.add_argument("--max-holes-ratio", type=float, default=1.15)
    parser.add_argument("--allow-observed", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    cadences = [int(value) for value in args.cadences.split(",")]
    thresholds = PromotionThresholds(
        min_score_delta=args.min_score_delta,
        min_direct_baseline_score=args.min_direct_baseline_score,
        min_attack_ratio=args.min_attack_ratio,
        max_danger_ratio=args.max_danger_ratio,
        max_holes_ratio=args.max_holes_ratio,
    )
    if (
        args.shortlist <= 0
        or args.seeds <= 0
        or args.horizon <= 0
        or args.threads <= 0
        or not cadences
        or any(cadence <= 0 for cadence in cadences)
    ):
        raise ValueError("selection counts, horizon, threads and cadences must be positive")
    if (
        not -1.0 <= thresholds.min_score_delta <= 1.0
        or not 0.0 <= thresholds.min_direct_baseline_score <= 1.0
        or min(
            thresholds.min_attack_ratio,
            thresholds.max_danger_ratio,
            thresholds.max_holes_ratio,
        )
        <= 0.0
    ):
        raise ValueError("invalid promotion thresholds")
    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    candidates = _discover_candidates(args.output_dir)
    shortlist = _shortlist_candidates(candidates, args.shortlist)
    anchors = list(dict.fromkeys(str(path) for path in args.anchor))
    reference = args.output_dir / "reference-model.pt"
    if reference.is_file() and args.baseline is None:
        anchors.append(str(reference))
    anchors = list(dict.fromkeys(anchors))
    baseline = str(args.baseline) if args.baseline is not None else None
    for path in [*shortlist, *anchors, *([baseline] if baseline is not None else [])]:
        if not Path(path).is_file():
            raise FileNotFoundError(f"selection checkpoint not found: {path}")
    opponents = list(
        dict.fromkeys([*anchors, *([baseline] if baseline is not None else []), *shortlist])
    )
    if not any(opponent != shortlist[0] for opponent in opponents):
        raise ValueError("champion selection requires at least one distinct opponent")

    cache: dict[str, LoadedVersusActor] = {}
    seeds = [args.base_seed + 104_729 * index for index in range(args.seeds)]
    evaluations = []
    summaries = []
    baseline_summary = None
    if baseline is not None:
        if not anchors:
            raise ValueError("baseline-gated selection requires fixed anchors")
        baseline_rows = _evaluate_rows(
            baseline,
            anchors,
            cache,
            seeds,
            args.horizon,
            cadences,
            args.allow_observed,
        )
        evaluations.extend(baseline_rows)
        baseline_summary = _candidate_summary(baseline, baseline_rows)
        print(json.dumps({"event": "baseline_summary", **baseline_summary}, sort_keys=True))
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
        if baseline is not None and baseline_summary is not None:
            fixed_summary = _candidate_summary(
                candidate_path,
                [row for row in candidate_rows if str(row["opponent"]) in anchors],
            )
            direct_rows = [row for row in candidate_rows if row["opponent"] == baseline]
            direct_score = (
                sum(float(row["score"]) for row in direct_rows) / len(direct_rows)
                if direct_rows
                else 0.0
            )
            summary.update(
                _promotion_gate(fixed_summary, baseline_summary, direct_score, thresholds)
            )
        summaries.append(summary)
        print(json.dumps({"event": "candidate_summary", **summary}, sort_keys=True))

    eligible = [summary for summary in summaries if summary.get("eligible", True)]
    baseline_retained = baseline is not None and not eligible
    selectable = eligible if eligible else summaries
    selected_summary = max(
        selectable,
        key=lambda row: (
            row["robust_score"],
            row["mean_score"],
            row["completion_rate"],
            row["outgoing_attack_per_piece"],
            -row["danger_rate"],
        ),
    )
    selected_source = Path(baseline if baseline_retained else str(selected_summary["checkpoint"]))
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
        "baseline": baseline,
        "baseline_summary": baseline_summary,
        "promotion_thresholds": asdict(thresholds),
        "baseline_retained": baseline_retained,
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
    unique_candidates = []
    seen_policies = set()
    for path in candidates:
        policy_fingerprint = _policy_fingerprint(path)
        if policy_fingerprint in seen_policies:
            continue
        seen_policies.add(policy_fingerprint)
        unique_candidates.append(str(path))
    return unique_candidates


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


def _evaluate_rows(
    candidate_path: str,
    opponent_paths: list[str],
    cache: dict[str, LoadedVersusActor],
    seeds: list[int],
    horizon: int,
    cadences: list[int],
    allow_observed: bool,
) -> list[dict[str, object]]:
    candidate = _load(cache, candidate_path, allow_observed)
    rows = []
    for opponent_path in opponent_paths:
        if opponent_path == candidate_path:
            continue
        opponent = _load(cache, opponent_path, allow_observed)
        for cadence in cadences:
            combined = _evaluate_pair(candidate, opponent, seeds, horizon, cadence)
            row = {
                "candidate": candidate_path,
                "opponent": opponent_path,
                "frames_per_placement": cadence,
                **asdict(combined),
                "score": combined.score,
                "completion_rate": combined.completion_rate,
            }
            rows.append(row)
            print(json.dumps({"event": "baseline_matchup", **row}, sort_keys=True))
    return rows


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
        "holes_per_piece": combined.holes_sum / pieces if pieces else 0.0,
    }


def _promotion_gate(
    candidate: dict[str, str | float | int],
    baseline: dict[str, str | float | int],
    direct_baseline_score: float,
    thresholds: PromotionThresholds,
) -> dict[str, bool | float]:
    score_delta = float(candidate["mean_score"]) - float(baseline["mean_score"])
    robust_score_delta = float(candidate["robust_score"]) - float(baseline["robust_score"])
    attack_ratio = _safe_ratio(
        float(candidate["outgoing_attack_per_piece"]),
        float(baseline["outgoing_attack_per_piece"]),
    )
    danger_ratio = _safe_ratio(float(candidate["danger_rate"]), float(baseline["danger_rate"]))
    holes_ratio = _safe_ratio(
        float(candidate["holes_per_piece"]), float(baseline["holes_per_piece"])
    )
    gates = {
        "score": score_delta >= thresholds.min_score_delta,
        "robust_score": robust_score_delta >= thresholds.min_score_delta,
        "direct_baseline_score": direct_baseline_score >= thresholds.min_direct_baseline_score,
        "attack": attack_ratio >= thresholds.min_attack_ratio,
        "danger": danger_ratio <= thresholds.max_danger_ratio,
        "holes": holes_ratio <= thresholds.max_holes_ratio,
    }
    return {
        "eligible": all(gates.values()),
        "gate_score": gates["score"],
        "gate_robust_score": gates["robust_score"],
        "gate_direct_baseline_score": gates["direct_baseline_score"],
        "gate_attack": gates["attack"],
        "gate_danger": gates["danger"],
        "gate_holes": gates["holes"],
        "fixed_score_delta": score_delta,
        "fixed_robust_score_delta": robust_score_delta,
        "direct_baseline_score": direct_baseline_score,
        "attack_ratio": attack_ratio,
        "danger_ratio": danger_ratio,
        "holes_ratio": holes_ratio,
    }


def _safe_ratio(numerator: float, denominator: float) -> float:
    if denominator > 0.0:
        return numerator / denominator
    return 1.0 if numerator <= 0.0 else 1.0e9


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _policy_fingerprint(path: Path) -> str:
    payload = torch.load(path, map_location="cpu", weights_only=True)
    model_state = payload["model_state"]
    digest = hashlib.sha256()
    for name in sorted(model_state):
        if name.startswith("value_core."):
            continue
        tensor = model_state[name].detach().cpu().contiguous()
        digest.update(name.encode("utf-8"))
        digest.update(str(tensor.dtype).encode("ascii"))
        digest.update(str(tuple(tensor.shape)).encode("ascii"))
        digest.update(tensor.numpy().tobytes())
    return digest.hexdigest()


if __name__ == "__main__":
    main()
