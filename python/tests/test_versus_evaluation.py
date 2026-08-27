import unittest

from tetris_rl.evaluation.versus import MatchSummary, _summary_payload, _wilson_interval


class VersusEvaluationMetricsTest(unittest.TestCase):
    def test_score_excludes_unfinished_games_and_reports_technique_rates(self) -> None:
        summary = MatchSummary(
            wins=3,
            losses=1,
            draws=0,
            unfinished=4,
            pieces=200,
            lines=80,
            attack=120,
            outgoing_attack=100,
            tetrises=4,
            t_spin_mini=2,
            t_spin_full=6,
            perfect_clears=1,
        )

        payload = _summary_payload(summary)

        self.assertEqual(summary.score, 0.75)
        self.assertEqual(summary.completion_rate, 0.5)
        self.assertEqual(payload["attack_per_piece"], 0.6)
        self.assertEqual(payload["tetris_per_100"], 2.0)
        self.assertEqual(payload["t_spin_full_per_100"], 3.0)

    def test_wilson_interval_contains_observed_rate(self) -> None:
        lower, upper = _wilson_interval(7, 10)

        self.assertLess(lower, 0.7)
        self.assertGreater(upper, 0.7)
        self.assertEqual(_wilson_interval(0, 0), (0.0, 1.0))


if __name__ == "__main__":
    unittest.main()
