import unittest
from dataclasses import asdict
from unittest.mock import patch

from tetris_rl.evaluation.versus import MatchSummary
from tetris_rl.evaluation.versus_select import _candidate_summary, _shortlist_candidates


class VersusChampionSelectionTest(unittest.TestCase):
    def test_candidate_summary_uses_worst_opponent_before_mean_score(self) -> None:
        rows = []
        for opponent, summary in (
            ("a.pt", MatchSummary(3, 1, 0, 0, pieces=100, outgoing_attack=20)),
            ("b.pt", MatchSummary(1, 3, 0, 0, pieces=100, outgoing_attack=30)),
        ):
            rows.append(
                {
                    "candidate": "candidate.pt",
                    "opponent": opponent,
                    "score": summary.score,
                    **asdict(summary),
                }
            )

        result = _candidate_summary("candidate.pt", rows)

        self.assertEqual(result["robust_score"], 0.25)
        self.assertEqual(result["mean_score"], 0.5)
        self.assertEqual(result["outgoing_attack_per_piece"], 0.25)

    def test_shortlist_keeps_metric_leaders_latest_and_midpoint(self) -> None:
        candidates = [f"candidate-{index}.pt" for index in range(10)]
        strengths = {
            checkpoint: (float(index), 0.0, 0.0) for index, checkpoint in enumerate(candidates)
        }
        with patch(
            "tetris_rl.evaluation.versus_select._training_robustness",
            side_effect=lambda checkpoint: strengths[checkpoint],
        ):
            shortlist = _shortlist_candidates(candidates, 5)

        self.assertEqual(shortlist[:3], candidates[-1:-4:-1])
        self.assertIn(candidates[len(candidates) // 2], shortlist)


if __name__ == "__main__":
    unittest.main()
