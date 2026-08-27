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

    @property
    def games(self) -> int:
        return self.wins + self.losses + self.draws + self.unfinished

    @property
    def score(self) -> float:
        return (self.wins + 0.5 * self.draws) / self.games


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
    )
    report = {
        "schema_version": "paired-versus-evaluation-v1",
        "candidate": str(args.candidate),
        "opponent": str(args.opponent),
        "base_seed": args.base_seed,
        "seeds": args.seeds,
        "horizon": args.horizon,
        "frames_per_placement": args.frames_per_placement,
        "candidate_left": asdict(candidate_left),
        "candidate_right": asdict(candidate_right),
        "combined": {**asdict(combined), "score": combined.score},
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
    for _ in range(horizon):
        if bool(torch.all(observation.done)):
            break
        selections = _greedy_selections(
            observation, candidate, opponent, candidate_side=candidate_side
        )
        observation = env.step(selections)
    candidate_results = observation.results[candidate_side::2]
    done = observation.done[candidate_side::2]
    return MatchSummary(
        wins=int(((candidate_results > 0) & done).sum().item()),
        losses=int(((candidate_results < 0) & done).sum().item()),
        draws=int(((candidate_results == 0) & done).sum().item()),
        unfinished=int((~done).sum().item()),
    )


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
