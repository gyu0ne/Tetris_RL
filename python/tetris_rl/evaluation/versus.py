import argparse
import json
from dataclasses import asdict, dataclass
from pathlib import Path

import torch

from tetris_rl.envs import VersusObservation, VersusVectorEnv
from tetris_rl.models import LoadedVersusActor, load_versus_actor


@dataclass(frozen=True)
class MatchSummary:
    wins: int
    losses: int
    draws: int
    unfinished: int
    pieces: int = 0
    lines: int = 0
    attack: int = 0
    outgoing_attack: int = 0
    tetrises: int = 0
    t_spin_mini: int = 0
    t_spin_full: int = 0
    perfect_clears: int = 0
    max_height_sum: int = 0
    holes_sum: int = 0
    pending_sum: int = 0
    ready_sum: int = 0
    danger_decisions: int = 0

    @property
    def games(self) -> int:
        return self.wins + self.losses + self.draws + self.unfinished

    @property
    def score(self) -> float:
        completed = self.wins + self.losses + self.draws
        return (self.wins + 0.5 * self.draws) / completed if completed else 0.0

    @property
    def completion_rate(self) -> float:
        return (self.games - self.unfinished) / self.games if self.games else 0.0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Paired closed-loop versus evaluation")
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--opponent", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--base-seed", type=int, default=80001)
    parser.add_argument("--seeds", type=int, default=256)
    parser.add_argument("--horizon", type=int, default=2_000)
    parser.add_argument("--frames-per-placement", type=int, default=12)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--allow-observed", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.seeds <= 0 or args.horizon <= 0 or args.threads <= 0:
        raise ValueError("seeds, horizon and threads must be positive")
    torch.set_num_threads(args.threads)
    candidate = load_versus_actor(args.candidate, allow_observed=args.allow_observed)
    opponent = load_versus_actor(args.opponent, allow_observed=args.allow_observed)
    seeds = [args.base_seed + 104_729 * index for index in range(args.seeds)]
    candidate_left = evaluate_side(
        candidate, opponent, seeds, args.horizon, args.frames_per_placement, candidate_side=0
    )
    candidate_right = evaluate_side(
        candidate, opponent, seeds, args.horizon, args.frames_per_placement, candidate_side=1
    )
    combined = MatchSummary(
        wins=candidate_left.wins + candidate_right.wins,
        losses=candidate_left.losses + candidate_right.losses,
        draws=candidate_left.draws + candidate_right.draws,
        unfinished=candidate_left.unfinished + candidate_right.unfinished,
        pieces=candidate_left.pieces + candidate_right.pieces,
        lines=candidate_left.lines + candidate_right.lines,
        attack=candidate_left.attack + candidate_right.attack,
        outgoing_attack=candidate_left.outgoing_attack + candidate_right.outgoing_attack,
        tetrises=candidate_left.tetrises + candidate_right.tetrises,
        t_spin_mini=candidate_left.t_spin_mini + candidate_right.t_spin_mini,
        t_spin_full=candidate_left.t_spin_full + candidate_right.t_spin_full,
        perfect_clears=candidate_left.perfect_clears + candidate_right.perfect_clears,
        max_height_sum=candidate_left.max_height_sum + candidate_right.max_height_sum,
        holes_sum=candidate_left.holes_sum + candidate_right.holes_sum,
        pending_sum=candidate_left.pending_sum + candidate_right.pending_sum,
        ready_sum=candidate_left.ready_sum + candidate_right.ready_sum,
        danger_decisions=candidate_left.danger_decisions + candidate_right.danger_decisions,
    )
    report = {
        "schema_version": "paired-versus-evaluation-v2",
        "candidate": str(args.candidate),
        "opponent": str(args.opponent),
        "base_seed": args.base_seed,
        "seeds": args.seeds,
        "horizon": args.horizon,
        "frames_per_placement": args.frames_per_placement,
        "candidate_left": _summary_payload(candidate_left),
        "candidate_right": _summary_payload(candidate_right),
        "combined": _summary_payload(combined),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, sort_keys=True))


def evaluate_side(
    candidate: LoadedVersusActor,
    opponent: LoadedVersusActor,
    seeds: list[int],
    horizon: int,
    frames_per_placement: int,
    *,
    candidate_side: int,
) -> MatchSummary:
    env = VersusVectorEnv(seeds, frames_per_placement)
    observation = env.observe()
    pieces = 0
    lines = 0
    attack = 0
    outgoing_attack = 0
    tetrises = 0
    t_spin_mini = 0
    t_spin_full = 0
    perfect_clears = 0
    max_height_sum = 0
    holes_sum = 0
    pending_sum = 0
    ready_sum = 0
    danger_decisions = 0
    for _ in range(horizon):
        if bool(torch.all(observation.done)):
            break
        selections = _greedy_selections(
            observation, candidate, opponent, candidate_side=candidate_side
        )
        for match_index in range(len(seeds)):
            decision = match_index * 2 + candidate_side
            selected = selections[decision]
            if selected is None:
                continue
            state = observation.state_features[decision]
            max_height = int(state[0].item())
            max_height_sum += max_height
            holes_sum += int(state[2].item())
            pending_sum += int(state[4].item())
            ready_sum += int(state[6].item())
            danger_decisions += int(max_height >= 16)
            diagnostic_index = observation.offsets[decision] + selected
            diagnostic = observation.candidate_diagnostics[diagnostic_index]
            cleared, spin, perfect, total_attack, sent_attack = (
                int(value.item()) for value in diagnostic
            )
            pieces += 1
            lines += cleared
            attack += total_attack
            outgoing_attack += sent_attack
            tetrises += int(cleared == 4)
            t_spin_mini += int(spin == 1)
            t_spin_full += int(spin == 2)
            perfect_clears += int(perfect != 0)
        observation = env.step(selections)
    candidate_results = observation.results[candidate_side::2]
    done = observation.done[candidate_side::2]
    return MatchSummary(
        wins=int(((candidate_results > 0) & done).sum().item()),
        losses=int(((candidate_results < 0) & done).sum().item()),
        draws=int(((candidate_results == 0) & done).sum().item()),
        unfinished=int((~done).sum().item()),
        pieces=pieces,
        lines=lines,
        attack=attack,
        outgoing_attack=outgoing_attack,
        tetrises=tetrises,
        t_spin_mini=t_spin_mini,
        t_spin_full=t_spin_full,
        perfect_clears=perfect_clears,
        max_height_sum=max_height_sum,
        holes_sum=holes_sum,
        pending_sum=pending_sum,
        ready_sum=ready_sum,
        danger_decisions=danger_decisions,
    )


def _summary_payload(summary: MatchSummary) -> dict[str, int | float | list[float]]:
    decisive = summary.wins + summary.losses
    interval = _wilson_interval(summary.wins, decisive)
    pieces = summary.pieces
    return {
        **asdict(summary),
        "score": summary.score,
        "completion_rate": summary.completion_rate,
        "decisive_win_rate": summary.wins / decisive if decisive else 0.0,
        "decisive_win_rate_95ci": list(interval),
        "lines_per_piece": summary.lines / pieces if pieces else 0.0,
        "attack_per_piece": summary.attack / pieces if pieces else 0.0,
        "outgoing_attack_per_piece": summary.outgoing_attack / pieces if pieces else 0.0,
        "cancelled_attack_per_piece": (summary.attack - summary.outgoing_attack) / pieces
        if pieces
        else 0.0,
        "mean_max_height": summary.max_height_sum / pieces if pieces else 0.0,
        "mean_holes": summary.holes_sum / pieces if pieces else 0.0,
        "mean_pending_garbage": summary.pending_sum / pieces if pieces else 0.0,
        "mean_ready_garbage": summary.ready_sum / pieces if pieces else 0.0,
        "danger_rate": summary.danger_decisions / pieces if pieces else 0.0,
        "tetris_per_100": 100.0 * summary.tetrises / pieces if pieces else 0.0,
        "t_spin_mini_per_100": 100.0 * summary.t_spin_mini / pieces if pieces else 0.0,
        "t_spin_full_per_100": 100.0 * summary.t_spin_full / pieces if pieces else 0.0,
        "perfect_clear_per_100": 100.0 * summary.perfect_clears / pieces if pieces else 0.0,
    }


def _wilson_interval(successes: int, trials: int) -> tuple[float, float]:
    if trials == 0:
        return (0.0, 1.0)
    z = 1.959963984540054
    proportion = successes / trials
    denominator = 1.0 + z * z / trials
    center = (proportion + z * z / (2.0 * trials)) / denominator
    margin = (
        z
        * ((proportion * (1.0 - proportion) + z * z / (4.0 * trials)) / trials) ** 0.5
        / denominator
    )
    return (max(0.0, center - margin), min(1.0, center + margin))


def _greedy_selections(
    observation: VersusObservation,
    candidate: LoadedVersusActor,
    opponent: LoadedVersusActor,
    *,
    candidate_side: int,
) -> list[int | None]:
    selections: list[int | None] = []
    with torch.no_grad():
        for decision in range(observation.decision_count):
            if bool(observation.done[decision]):
                selections.append(None)
                continue
            start, end = observation.offsets[decision : decision + 2]
            actor = candidate if decision % 2 == candidate_side else opponent
            logits = actor.model.actor_logits(observation.candidate_features[start:end])
            selections.append(int(torch.argmax(logits).item()))
    return selections


if __name__ == "__main__":
    main()
