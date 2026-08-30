import unittest

from tetris_rl.training.opponent_pool import OpponentPoolState


class StableOpponentPoolTest(unittest.TestCase):
    def test_promotion_changes_at_most_one_membership_and_preserves_anchor(self) -> None:
        pool = OpponentPoolState.initialize(["anchor.pt"])
        previous = set(pool.checkpoints())
        for update in range(10, 100, 10):
            event = pool.promote(
                f"update-{update:03d}.pt",
                update,
                limit=6,
                recent_slots=2,
            )
            current = set(pool.checkpoints())
            self.assertLessEqual(len(previous - current), 1)
            self.assertIn("anchor.pt", current)
            self.assertEqual(event["added"], f"update-{update:03d}.pt")
            previous = current
        self.assertEqual(len(pool.members), 6)
        self.assertEqual(sum(member.role == "recent" for member in pool.members), 2)

    def test_timestamped_results_decay_toward_unknown_score(self) -> None:
        pool = OpponentPoolState.initialize(["anchor.pt"])
        for _ in range(8):
            pool.record_result("anchor.pt", 0, 0.0, history_limit=32)
        initial_score, initial_games = pool.estimate("anchor.pt", 0, 100.0)
        old_score, old_games = pool.estimate("anchor.pt", 300, 100.0)
        self.assertLess(initial_score, old_score)
        self.assertLess(old_score, 0.5)
        self.assertAlmostEqual(initial_games, 8.0)
        self.assertAlmostEqual(old_games, 1.0)

    def test_mixed_sampler_is_deterministic_and_keeps_uniform_coverage(self) -> None:
        pool = OpponentPoolState.initialize(["a.pt", "b.pt", "c.pt"])
        for _ in range(20):
            pool.record_result("a.pt", 10, 0.0, history_limit=64)
            pool.record_result("c.pt", 10, 1.0, history_limit=64)
        kwargs = {
            "half_life_updates": 100.0,
            "balanced_fraction": 0.4,
            "hard_fraction": 0.3,
            "uniform_fraction": 0.3,
            "exponent": 1.0,
            "min_weight": 0.05,
        }
        first = [pool.sample(seed, 10, **kwargs) for seed in range(500)]
        second = [pool.sample(seed, 10, **kwargs) for seed in range(500)]
        self.assertEqual(first, second)
        checkpoints = [checkpoint for checkpoint, _ in first]
        self.assertTrue(
            all(checkpoints.count(checkpoint) > 20 for checkpoint in pool.checkpoints())
        )
        self.assertEqual({mode for _, mode in first}, {"balanced", "hard", "uniform"})

    def test_round_trip_preserves_membership_and_results(self) -> None:
        pool = OpponentPoolState.initialize(["anchor.pt"])
        pool.promote("candidate.pt", 50, limit=4, recent_slots=2)
        pool.record_result("candidate.pt", 60, 1.0, history_limit=8)
        restored = OpponentPoolState.from_payload(pool.to_payload())
        self.assertEqual(restored.to_payload(), pool.to_payload())


if __name__ == "__main__":
    unittest.main()
