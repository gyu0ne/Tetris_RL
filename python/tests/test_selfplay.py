import unittest
from pathlib import Path
from typing import cast

import torch
from tetris_rl.envs import VersusObservation
from tetris_rl.models import VersusActorCritic
from tetris_rl.training.selfplay import (
    OpponentPoolEntry,
    SelfPlayConfig,
    _batched_actor_logits,
    _entropy_coefficient,
    _kickstart_coefficient,
    _new_match_assignment,
    _ragged_policy_terms,
    _ragged_target_kl,
    _resolve_actor_assignments,
    _sample_pfsp_opponent,
    _segmented_argmax,
    _segmented_log_probabilities,
    _segmented_unit_range,
    _stratified_paths,
    _tactical_curriculum_coefficient,
)
from torch.distributions import Categorical


class CountingActor:
    def __init__(self, scale: float) -> None:
        self.scale = scale
        self.calls = 0

    def actor_logits(self, candidates: torch.Tensor) -> torch.Tensor:
        self.calls += 1
        return candidates[:, 0] * self.scale


class SelfPlayOpponentPoolTest(unittest.TestCase):
    def test_vectorized_ragged_policy_terms_match_categorical_reference(self) -> None:
        logits = torch.tensor([0.2, -0.5, 1.1, 0.3, -0.7, 0.4], requires_grad=True)
        teacher_logits = torch.tensor([-0.1, 0.7, 0.2, -0.4, 0.9, 0.1])
        counts = torch.tensor([2, 3, 1])
        actions = torch.tensor([1, 2, 0])
        teacher_log_probability = _segmented_log_probabilities(teacher_logits, counts)

        selected, entropy, normalized_entropy, kickstart, _ = _ragged_policy_terms(
            logits, teacher_log_probability, counts, actions
        )
        offset = 0
        expected_selected = []
        expected_entropy = []
        expected_normalized = []
        expected_kickstart = []
        for count, action in zip(counts.tolist(), actions.tolist(), strict=True):
            end = offset + count
            student = Categorical(logits=logits[offset:end])
            teacher_log_probability = torch.log_softmax(teacher_logits[offset:end], dim=0)
            teacher_probability = teacher_log_probability.exp()
            student_log_probability = torch.log_softmax(logits[offset:end], dim=0)
            expected_selected.append(student.log_prob(torch.tensor(action)))
            expected_entropy.append(student.entropy())
            expected_normalized.append(
                student.entropy() / torch.log(torch.tensor(float(count)))
                if count > 1
                else torch.tensor(0.0)
            )
            expected_kickstart.append(
                torch.sum(teacher_probability * (teacher_log_probability - student_log_probability))
            )
            offset = end

        self.assertTrue(torch.allclose(selected, torch.stack(expected_selected)))
        self.assertTrue(torch.allclose(entropy, torch.stack(expected_entropy)))
        self.assertTrue(torch.allclose(normalized_entropy, torch.stack(expected_normalized)))
        self.assertTrue(torch.allclose(kickstart, torch.stack(expected_kickstart)))
        (selected.mean() + entropy.mean() + kickstart.mean()).backward()
        self.assertTrue(torch.isfinite(logits.grad).all())

    def test_actor_inference_batches_each_distinct_model_once(self) -> None:
        learner = CountingActor(1.0)
        opponent = CountingActor(-1.0)
        observation = VersusObservation(
            candidate_features=torch.tensor([[1.0], [2.0], [3.0], [4.0], [5.0], [6.0]]),
            candidate_diagnostics=torch.empty((6, 0)),
            state_features=torch.empty((3, 0)),
            offsets=(0, 2, 5, 6),
            done=torch.zeros(3, dtype=torch.bool),
            results=torch.zeros(3),
        )

        logits = _batched_actor_logits(
            observation,
            [None, cast(VersusActorCritic, opponent), None],
            cast(VersusActorCritic, learner),
        )

        self.assertEqual(learner.calls, 1)
        self.assertEqual(opponent.calls, 1)
        self.assertTrue(torch.equal(logits[0], torch.tensor([1.0, 2.0])))
        self.assertTrue(torch.equal(logits[1], torch.tensor([-3.0, -4.0, -5.0])))
        self.assertTrue(torch.equal(logits[2], torch.tensor([6.0])))

    def test_ragged_target_kl_matches_categorical_reference(self) -> None:
        student_logits = torch.tensor([0.2, -0.5, 1.1, 0.3, -0.7], requires_grad=True)
        target_logits = torch.tensor([-0.1, 0.7, 0.2, -0.4, 0.9])
        counts = torch.tensor([2, 3])

        actual = _ragged_target_kl(student_logits, target_logits, counts)
        expected = []
        offset = 0
        for count in counts.tolist():
            end = offset + count
            target_log = torch.log_softmax(target_logits[offset:end], dim=0)
            student_log = torch.log_softmax(student_logits[offset:end], dim=0)
            expected.append(torch.sum(target_log.exp() * (target_log - student_log)))
            offset = end

        self.assertTrue(torch.allclose(actual, torch.stack(expected)))
        actual.mean().backward()
        self.assertTrue(torch.isfinite(student_logits.grad).all())

    def test_segmented_tactical_ranking_is_scale_independent(self) -> None:
        values = torch.tensor([100.0, 50.0, 0.0, -2.0, -1.0])
        counts = torch.tensor([3, 2])

        normalized = _segmented_unit_range(values, counts)
        torch.testing.assert_close(normalized, torch.tensor([1.0, 0.5, 0.0, 0.0, 1.0]))
        torch.testing.assert_close(_segmented_argmax(values, counts), torch.tensor([0, 1]))

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

    def test_v4_tactical_curriculum_decays_to_zero(self) -> None:
        config = SelfPlayConfig(
            schema_version="versus-selfplay-ppo-v4",
            frames_per_placement=12,
            parallel_matches=2,
            rollout_steps=4,
            ppo_epochs=1,
            minibatch_decisions=8,
            learning_rate=0.0001,
            gamma=0.9995,
            gae_lambda=0.999,
            clip_ratio=0.2,
            entropy_coefficient=0.0003,
            value_coefficient=0.5,
            max_grad_norm=0.5,
            shaping_scale=0.05,
            base_seed=1,
            seed_stride=104729,
            snapshot_interval_updates=1,
            self_play_fraction=0.35,
            historical_fraction=0.5,
            opponent_pool_limit=32,
            entropy_coefficient_final=0.00003,
            entropy_decay_updates=500,
            normalize_entropy=True,
            model_architecture="joint-residual-v2",
            solo_learning_rate_multiplier=0.1,
            kickstart_coefficient=0.02,
            kickstart_coefficient_final=0.001,
            kickstart_decay_updates=500,
            tactical_potential_fraction=0.25,
            tactical_curriculum_coefficient=0.0001,
            tactical_curriculum_coefficient_final=0.0,
            tactical_curriculum_decay_updates=300,
            tactical_curriculum_temperature=1.0,
        )
        config.validate()
        self.assertAlmostEqual(_tactical_curriculum_coefficient(config, 0), 0.0001)
        self.assertAlmostEqual(_tactical_curriculum_coefficient(config, 150), 0.00005)
        self.assertAlmostEqual(_tactical_curriculum_coefficient(config, 300), 0.0)

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
