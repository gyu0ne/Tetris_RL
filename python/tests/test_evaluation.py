import unittest

from tetris_rl.evaluation.closed_loop import evaluate_closed_loop
from tetris_rl.evaluation.offline import evaluate_decisions
from tetris_rl.models import AfterstateScorer
from tetris_rl.models.checkpoint import LoadedScorer
from tetris_rl.training.dataset import Decision
from torch import Tensor


class FirstFeatureScorer:
    def score(self, features: Tensor) -> Tensor:
        return features[:, 0]


class FakeSoloBatch:
    def __init__(self) -> None:
        self.pieces = [0, 0]

    def candidates(self) -> tuple[bytes, list[int], list[bool]]:
        done = [False, self.pieces[1] >= 1]
        active = sum(not value for value in done)
        features = b"\0" * (active * 10 * 4)
        offsets = [0]
        for is_done in done:
            offsets.append(offsets[-1] + int(not is_done))
        return features, offsets, done

    def step(self, selections: list[int]) -> None:
        for index, selected in enumerate(selections):
            if selected >= 0:
                self.pieces[index] += 1

    def pieces_placed(self) -> list[int]:
        return self.pieces


class OfflineEvaluationTest(unittest.TestCase):
    def test_ties_are_optimal_without_matching_teacher_tiebreak(self) -> None:
        zero = (0.0,) * 10
        one = (1.0,) + (0.0,) * 9
        decisions = [
            Decision("tie", 5, (zero, one), (10.0, 10.0), 0),
            Decision("unique", 10, (zero, one), (10.0, 0.0), 0),
        ]

        metrics = evaluate_decisions(FirstFeatureScorer(), decisions, batch_decisions=2)

        self.assertEqual(metrics.decisions, 2)
        self.assertEqual(metrics.positive_margin_decisions, 1)
        self.assertEqual(metrics.tie_aware_optimal_rate, 0.5)
        self.assertEqual(metrics.positive_margin_agreement, 0.0)
        self.assertEqual(metrics.mean_teacher_regret, 5.0)
        self.assertEqual(metrics.mean_positive_margin, 10.0)
        self.assertEqual(metrics.mean_normalized_regret, 0.5)

    def test_empty_input_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "at least one decision"):
            evaluate_decisions(FirstFeatureScorer(), [], batch_decisions=2)


class ClosedLoopEvaluationTest(unittest.TestCase):
    def test_survival_uses_model_actions_and_engine_piece_counts(self) -> None:
        model = AfterstateScorer()
        scorer = LoadedScorer(
            model,
            Tensor([0.0] * 10),
            Tensor([1.0] * 10),
            {},
        )

        metrics = evaluate_closed_loop(scorer, FakeSoloBatch(), horizon=2)

        self.assertEqual(metrics.seeds, 2)
        self.assertEqual(metrics.survived, 1)
        self.assertEqual(metrics.survival_at_horizon, 0.5)
        self.assertEqual(metrics.mean_pieces_placed, 1.5)


if __name__ == "__main__":
    unittest.main()
