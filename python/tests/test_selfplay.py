import unittest
from pathlib import Path
from typing import cast

from tetris_rl.models import VersusActorCritic
from tetris_rl.training.selfplay import (
    OpponentPoolEntry,
    SelfPlayConfig,
    _entropy_coefficient,
    _kickstart_coefficient,
    _new_match_assignment,
    _resolve_actor_assignments,
    _sample_pfsp_opponent,
    _stratified_paths,
)


class SelfPlayOpponentPoolTest(unittest.TestCase):
    def test_assignments_use_current_historical_and_bootstrap_ratios(self) -> None:
        config = SelfPlayConfig(
            schema_version="versus-selfplay-ppo-v2",
            frames_per_placement=12,
            parallel_matches=10,
            rollout_steps=4,
            ppo_epochs=1,
            minibatch_decisions=8,
            learning_rate=0.0001,
            gamma=0.997,
            gae_lambda=0.95,
            clip_ratio=0.2,
            entropy_coefficient=0.002,
            value_coefficient=0.5,
            max_grad_norm=0.5,
            shaping_scale=0.1,
            base_seed=1,
            seed_stride=104729,
            snapshot_interval_updates=1,
            self_play_fraction=0.5,
            historical_fraction=0.3,
            opponent_pool_limit=8,
            entropy_coefficient_final=0.0002,
            entropy_decay_updates=200,
            normalize_entropy=True,
        )
        bootstrap = cast(VersusActorCritic, object())
        historical = cast(VersusActorCritic, object())
        pool = [OpponentPoolEntry(checkpoint="history.pt", model=historical)]

        assignments = [
            _new_match_assignment(config, index, 101 + index, pool)
            for index in range(config.parallel_matches)
        ]
        actors, _ = _resolve_actor_assignments(assignments, bootstrap, pool, False)

        self.assertTrue(all(item.kind == "self_play" for item in assignments[:5]))
        self.assertTrue(all(item.kind == "historical" for item in assignments[5:8]))
        self.assertTrue(all(item.kind == "bootstrap" for item in assignments[8:]))
        self.assertEqual(sum(actor is historical for actor in actors), 3)
        self.assertEqual(sum(actor is bootstrap for actor in actors), 2)
        self.assertEqual(sum(actor is None for actor in actors), 15)

    def test_entropy_coefficient_decays_to_the_declared_floor(self) -> None:
        config = SelfPlayConfig(
            schema_version="versus-selfplay-ppo-v2",
            frames_per_placement=12,
            parallel_matches=2,
            rollout_steps=4,
            ppo_epochs=1,
            minibatch_decisions=8,
            learning_rate=0.0001,
            gamma=0.997,
            gae_lambda=0.95,
            clip_ratio=0.2,
            entropy_coefficient=0.002,
            value_coefficient=0.5,
            max_grad_norm=0.5,
            shaping_scale=0.1,
            base_seed=1,
            seed_stride=104729,
            snapshot_interval_updates=1,
            self_play_fraction=0.5,
            historical_fraction=0.0,
            opponent_pool_limit=2,
            entropy_coefficient_final=0.0002,
            entropy_decay_updates=200,
            normalize_entropy=True,
        )

        self.assertAlmostEqual(_entropy_coefficient(config, 0), 0.002)
        self.assertAlmostEqual(_entropy_coefficient(config, 100), 0.0011)
        self.assertAlmostEqual(_entropy_coefficient(config, 200), 0.0002)
        self.assertAlmostEqual(_entropy_coefficient(config, 500), 0.0002)

    def test_v3_auxiliary_coefficient_decays_and_history_is_stratified(self) -> None:
        config = SelfPlayConfig(
            schema_version="versus-selfplay-ppo-v3",
            frames_per_placement=12,
            parallel_matches=2,
            rollout_steps=4,
            ppo_epochs=1,
            minibatch_decisions=8,
            learning_rate=0.0001,
            gamma=0.9995,
            gae_lambda=0.995,
            clip_ratio=0.2,
            entropy_coefficient=0.0003,
            value_coefficient=0.5,
            max_grad_norm=0.5,
            shaping_scale=0.05,
            base_seed=1,
            seed_stride=104729,
            snapshot_interval_updates=1,
            self_play_fraction=0.5,
            historical_fraction=0.5,
            opponent_pool_limit=3,
            entropy_coefficient_final=0.00003,
            entropy_decay_updates=100,
            normalize_entropy=True,
            model_architecture="joint-residual-v2",
            solo_learning_rate_multiplier=0.1,
            kickstart_coefficient=0.02,
            kickstart_coefficient_final=0.001,
            kickstart_decay_updates=100,
        )
        self.assertAlmostEqual(_kickstart_coefficient(config, 0), 0.02)
        self.assertAlmostEqual(_kickstart_coefficient(config, 100), 0.001)
        paths = [Path(f"{index}.pt") for index in range(10)]
        self.assertEqual(_stratified_paths(paths, 3), [paths[0], paths[4], paths[9]])

    def test_pfsp_samples_hard_opponent_more_often(self) -> None:
        config = SelfPlayConfig(
            schema_version="versus-selfplay-ppo-v2",
            frames_per_placement=12,
            parallel_matches=2,
            rollout_steps=4,
            ppo_epochs=1,
            minibatch_decisions=8,
            learning_rate=0.0001,
            gamma=0.997,
            gae_lambda=0.95,
            clip_ratio=0.2,
            entropy_coefficient=0.002,
            value_coefficient=0.5,
            max_grad_norm=0.5,
            shaping_scale=0.1,
            base_seed=1,
            seed_stride=104729,
            snapshot_interval_updates=1,
            self_play_fraction=0.5,
            historical_fraction=0.5,
            opponent_pool_limit=2,
            entropy_coefficient_final=0.0002,
            entropy_decay_updates=200,
            normalize_entropy=True,
        )
        model = cast(VersusActorCritic, object())
        pool = [
            OpponentPoolEntry(checkpoint="hard.pt", model=model),
            OpponentPoolEntry(checkpoint="easy.pt", model=model),
        ]
        stats = {
            "hard.pt": {"games": 100, "wins": 10, "losses": 90, "draws": 0},
            "easy.pt": {"games": 100, "wins": 90, "losses": 10, "draws": 0},
        }
        choices = [
            _sample_pfsp_opponent(config, seed, pool, stats).checkpoint for seed in range(200)
        ]
        self.assertGreater(choices.count("hard.pt"), 150)


if __name__ == "__main__":
    unittest.main()
