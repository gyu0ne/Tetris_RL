import unittest
from typing import cast

from tetris_rl.models import VersusActorCritic
from tetris_rl.training.selfplay import SelfPlayConfig, _opponent_assignments


class SelfPlayOpponentPoolTest(unittest.TestCase):
    def test_assignments_use_current_historical_and_bootstrap_ratios(self) -> None:
        config = SelfPlayConfig(
            schema_version="versus-selfplay-ppo-v1",
            frames_per_placement=12,
            parallel_matches=10,
            rollout_steps=4,
            ppo_epochs=1,
            minibatch_decisions=8,
            learning_rate=0.0001,
            gamma=0.997,
            gae_lambda=0.95,
            clip_ratio=0.2,
            entropy_coefficient=0.01,
            value_coefficient=0.5,
            max_grad_norm=0.5,
            shaping_scale=0.1,
            base_seed=1,
            seed_stride=104729,
            snapshot_interval_updates=1,
            self_play_fraction=0.5,
            historical_fraction=0.3,
            opponent_pool_limit=8,
        )
        bootstrap = cast(VersusActorCritic, object())
        historical = cast(VersusActorCritic, object())

        assignments = _opponent_assignments(config, 0, bootstrap, [historical])

        self.assertEqual(assignments[:10], [None] * 10)
        self.assertEqual(sum(actor is historical for actor in assignments), 3)
        self.assertEqual(sum(actor is bootstrap for actor in assignments), 2)
        self.assertEqual(sum(actor is None for actor in assignments), 15)


if __name__ == "__main__":
    unittest.main()
