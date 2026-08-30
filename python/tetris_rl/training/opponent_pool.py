from __future__ import annotations

import math
import random
from dataclasses import dataclass, field

POOL_SCHEMA = "stable-opponent-pool-v1"
VALID_ROLES = {"anchor", "recent", "archive"}
VALID_SAMPLING_MODES = {"balanced", "hard", "uniform"}


@dataclass(frozen=True)
class TimedResult:
    update: int
    score: float

    def validate(self) -> None:
        if self.update < 0 or self.score not in {0.0, 0.5, 1.0}:
            raise ValueError("invalid timestamped opponent result")


@dataclass
class PoolMember:
    checkpoint: str
    role: str
    added_update: int
    protected: bool = False
    results: list[TimedResult] = field(default_factory=list)

    def validate(self) -> None:
        if not self.checkpoint or self.role not in VALID_ROLES or self.added_update < 0:
            raise ValueError("invalid opponent-pool member")
        if self.protected and self.role != "anchor":
            raise ValueError("protected opponent must be an anchor")
        for result in self.results:
            result.validate()


@dataclass
class OpponentPoolState:
    members: list[PoolMember] = field(default_factory=list)
    promoted_checkpoints: list[str] = field(default_factory=list)

    @classmethod
    def initialize(cls, anchors: list[str], update: int = 0) -> OpponentPoolState:
        unique = list(dict.fromkeys(anchors))
        state = cls(
            members=[
                PoolMember(
                    checkpoint=checkpoint,
                    role="anchor",
                    added_update=update,
                    protected=True,
                )
                for checkpoint in unique
            ],
            promoted_checkpoints=list(unique),
        )
        state.validate()
        return state

    @classmethod
    def from_payload(cls, payload: dict[str, object]) -> OpponentPoolState:
        if payload.get("schema_version") != POOL_SCHEMA:
            raise ValueError("unsupported opponent-pool state schema")
        members = []
        raw_members = payload.get("members")
        if not isinstance(raw_members, list):
            raise ValueError("opponent-pool members must be a list")
        for raw_member in raw_members:
            if not isinstance(raw_member, dict):
                raise ValueError("opponent-pool member must be an object")
            raw_results = raw_member.get("results", [])
            if not isinstance(raw_results, list):
                raise ValueError("opponent result history must be a list")
            results = [
                TimedResult(update=int(item["update"]), score=float(item["score"]))
                for item in raw_results
                if isinstance(item, dict)
            ]
            if len(results) != len(raw_results):
                raise ValueError("opponent result history contains a non-object")
            members.append(
                PoolMember(
                    checkpoint=str(raw_member["checkpoint"]),
                    role=str(raw_member["role"]),
                    added_update=int(raw_member["added_update"]),
                    protected=bool(raw_member.get("protected", False)),
                    results=results,
                )
            )
        promoted = payload.get("promoted_checkpoints", [])
        if not isinstance(promoted, list):
            raise ValueError("promoted checkpoints must be a list")
        state = cls(members=members, promoted_checkpoints=[str(item) for item in promoted])
        state.validate()
        return state

    def to_payload(self) -> dict[str, object]:
        return {
            "schema_version": POOL_SCHEMA,
            "members": [
                {
                    "checkpoint": member.checkpoint,
                    "role": member.role,
                    "added_update": member.added_update,
                    "protected": member.protected,
                    "results": [
                        {"update": result.update, "score": result.score}
                        for result in member.results
                    ],
                }
                for member in self.members
            ],
            "promoted_checkpoints": list(self.promoted_checkpoints),
        }

    def validate(self) -> None:
        checkpoints = [member.checkpoint for member in self.members]
        if len(checkpoints) != len(set(checkpoints)):
            raise ValueError("opponent pool contains duplicate checkpoints")
        if len(self.promoted_checkpoints) != len(set(self.promoted_checkpoints)):
            raise ValueError("promoted checkpoint history contains duplicates")
        for member in self.members:
            member.validate()

    def checkpoints(self) -> list[str]:
        return [member.checkpoint for member in self.members]

    def promote(
        self,
        checkpoint: str,
        update: int,
        *,
        limit: int,
        recent_slots: int,
    ) -> dict[str, object]:
        if checkpoint in self.promoted_checkpoints:
            return {"added": None, "removed": None, "demoted": None}
        if limit <= 0 or recent_slots <= 0 or recent_slots >= limit:
            raise ValueError("invalid stable opponent-pool capacity")
        if sum(member.protected for member in self.members) > limit - recent_slots:
            raise ValueError("protected anchors leave no archive capacity")

        self.promoted_checkpoints.append(checkpoint)
        self.members.append(PoolMember(checkpoint=checkpoint, role="recent", added_update=update))
        demoted: str | None = None
        recent = sorted(
            (member for member in self.members if member.role == "recent"),
            key=lambda member: (member.added_update, member.checkpoint),
        )
        if len(recent) > recent_slots:
            recent[0].role = "archive"
            demoted = recent[0].checkpoint

        removed: str | None = None
        if len(self.members) > limit:
            candidates = [
                member
                for member in self.members
                if member.role == "archive" and not member.protected
            ]
            if not candidates:
                raise ValueError("opponent pool has no removable archive member")
            victim = min(candidates, key=self._redundancy_key)
            removed = victim.checkpoint
            self.members.remove(victim)
        self.validate()
        return {"added": checkpoint, "removed": removed, "demoted": demoted}

    def record_result(
        self, checkpoint: str, update: int, score: float, *, history_limit: int
    ) -> None:
        member = next((item for item in self.members if item.checkpoint == checkpoint), None)
        if member is None:
            return
        result = TimedResult(update=update, score=score)
        result.validate()
        member.results.append(result)
        member.results = member.results[-history_limit:]

    def estimate(
        self, checkpoint: str, current_update: int, half_life_updates: float
    ) -> tuple[float, float]:
        member = next(item for item in self.members if item.checkpoint == checkpoint)
        weighted_score = 0.0
        effective_games = 0.0
        for result in member.results:
            age = max(current_update - result.update, 0)
            weight = 2.0 ** (-age / half_life_updates)
            weighted_score += weight * result.score
            effective_games += weight
        return (1.0 + weighted_score) / (2.0 + effective_games), effective_games

    def sample(
        self,
        seed: int,
        current_update: int,
        *,
        half_life_updates: float,
        balanced_fraction: float,
        hard_fraction: float,
        uniform_fraction: float,
        exponent: float,
        min_weight: float,
    ) -> tuple[str, str]:
        if not self.members:
            raise ValueError("cannot sample an empty opponent pool")
        chooser = random.Random(seed ^ 0x5EED5EED)
        channel = chooser.random()
        if channel < balanced_fraction:
            mode = "balanced"
        elif channel < balanced_fraction + hard_fraction:
            mode = "hard"
        else:
            mode = "uniform"
        weights = self._mode_weights(
            current_update,
            half_life_updates=half_life_updates,
            mode=mode,
            exponent=exponent,
            min_weight=min_weight,
        )
        member = chooser.choices(self.members, weights=weights, k=1)[0]
        return member.checkpoint, mode

    def metrics(
        self,
        current_update: int,
        *,
        half_life_updates: float,
        balanced_fraction: float,
        hard_fraction: float,
        uniform_fraction: float,
        exponent: float,
        min_weight: float,
    ) -> dict[str, float | int]:
        if not self.members:
            return {
                "opponent_pool_size": 0,
                "opponent_pool_effective_size": 0.0,
                "opponent_pool_mean_score": 0.0,
                "opponent_pool_min_score": 0.0,
                "opponent_pool_max_score": 0.0,
                "opponent_pool_mean_effective_games": 0.0,
                "opponent_pool_max_sample_probability": 0.0,
            }
        estimates = [
            self.estimate(member.checkpoint, current_update, half_life_updates)
            for member in self.members
        ]
        probabilities = [0.0] * len(self.members)
        for fraction, mode in (
            (balanced_fraction, "balanced"),
            (hard_fraction, "hard"),
            (uniform_fraction, "uniform"),
        ):
            weights = self._mode_weights(
                current_update,
                half_life_updates=half_life_updates,
                mode=mode,
                exponent=exponent,
                min_weight=min_weight,
            )
            total = sum(weights)
            for index, weight in enumerate(weights):
                probabilities[index] += fraction * weight / total
        return {
            "opponent_pool_size": len(self.members),
            "opponent_pool_anchor_count": sum(member.protected for member in self.members),
            "opponent_pool_recent_count": sum(member.role == "recent" for member in self.members),
            "opponent_pool_effective_size": 1.0
            / sum(probability * probability for probability in probabilities),
            "opponent_pool_mean_score": sum(score for score, _ in estimates) / len(estimates),
            "opponent_pool_min_score": min(score for score, _ in estimates),
            "opponent_pool_max_score": max(score for score, _ in estimates),
            "opponent_pool_mean_effective_games": sum(games for _, games in estimates)
            / len(estimates),
            "opponent_pool_max_sample_probability": max(probabilities),
        }

    def _mode_weights(
        self,
        current_update: int,
        *,
        half_life_updates: float,
        mode: str,
        exponent: float,
        min_weight: float,
    ) -> list[float]:
        if mode not in VALID_SAMPLING_MODES:
            raise ValueError("unknown opponent sampling mode")
        weights = []
        for member in self.members:
            score, _ = self.estimate(member.checkpoint, current_update, half_life_updates)
            if mode == "balanced":
                raw = (4.0 * score * (1.0 - score)) ** exponent
            elif mode == "hard":
                raw = (1.0 - score) ** exponent
            else:
                raw = 1.0
            weights.append(max(min_weight, raw))
        return weights

    def _redundancy_key(self, member: PoolMember) -> tuple[int, int, str]:
        other_updates = [item.added_update for item in self.members if item is not member]
        nearest_gap = min(
            (abs(member.added_update - update) for update in other_updates),
            default=math.inf,
        )
        return int(nearest_gap), member.added_update, member.checkpoint
