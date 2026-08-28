import tempfile
import unittest
from pathlib import Path

import torch
from tetris_rl.features import FEATURE_NAMES, MECHANICS_STATUS, SCHEMA_VERSION
from tetris_rl.models import (
    AfterstateScorer,
    VersusActorCritic,
    load_scorer,
    load_versus_actor,
)


class AfterstateScorerTest(unittest.TestCase):
    def test_shared_scorer_returns_one_logit_per_candidate(self) -> None:
        model = AfterstateScorer()
        candidates = torch.zeros((7, 10), dtype=torch.float32)

        logits = model(candidates)

        self.assertEqual(tuple(logits.shape), (7,))
        self.assertEqual(model.parameter_count(), 2_817)

    def test_wrong_feature_width_is_rejected(self) -> None:
        model = AfterstateScorer()
        with self.assertRaises(ValueError):
            model(torch.zeros((2, 9), dtype=torch.float32))

    def test_self_contained_checkpoint_load_requires_observed_opt_in(self) -> None:
        model = AfterstateScorer()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "model.pt"
            torch.save(
                {
                    "checkpoint_schema": "afterstate-scorer-v1",
                    "dataset_schema": SCHEMA_VERSION,
                    "feature_names": FEATURE_NAMES,
                    "mechanics_status": MECHANICS_STATUS,
                    "model_config": model.config.to_dict(),
                    "model_state": model.state_dict(),
                    "feature_mean": (0.0,) * len(FEATURE_NAMES),
                    "feature_std": (1.0,) * len(FEATURE_NAMES),
                },
                path,
            )

            with self.assertRaises(ValueError):
                load_scorer(path)
            loaded = load_scorer(path, allow_observed=True)
            scores = loaded.score(torch.zeros((3, len(FEATURE_NAMES))))

            self.assertEqual(tuple(scores.shape), (3,))

    def test_versus_bootstrap_initially_matches_solo_logits_and_has_odd_value(self) -> None:
        model = AfterstateScorer()
        from tetris_rl.models.checkpoint import LoadedScorer

        solo = LoadedScorer(
            model=model,
            feature_mean=torch.arange(10, dtype=torch.float32),
            feature_std=torch.arange(1, 11, dtype=torch.float32),
            metadata={},
        )
        versus = VersusActorCritic(solo)
        candidates = torch.randn((11, 76))
        state = torch.randn((1, 122))
        swapped = state.clone()
        swapped[:, :12] = state[:, :12].reshape(1, 6, 2).flip(-1).reshape(1, 12)
        for own, opponent in ((12, 22), (32, 42), (52, 87)):
            width = opponent - own
            swapped[:, own : own + width] = state[:, opponent : opponent + width]
            swapped[:, opponent : opponent + width] = state[:, own : own + width]

        expected = model((candidates[:, :10] - solo.feature_mean) / solo.feature_std)
        torch.testing.assert_close(versus.actor_logits(candidates), expected)
        torch.testing.assert_close(versus.value(state), -versus.value(swapped))

    def test_self_contained_versus_checkpoint_loads_without_solo_file(self) -> None:
        from dataclasses import asdict

        from tetris_rl.models.checkpoint import LoadedScorer

        solo = LoadedScorer(
            model=AfterstateScorer(),
            feature_mean=torch.zeros(10),
            feature_std=torch.ones(10),
            metadata={},
        )
        model = VersusActorCritic(solo)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "versus.pt"
            torch.save(
                {
                    "checkpoint_schema": "versus-actor-critic-v2",
                    "mechanics_status": MECHANICS_STATUS,
                    "model_config": asdict(model.config),
                    "solo_model_config": model.solo_model.config.to_dict(),
                    "model_state": model.state_dict(),
                },
                path,
            )

            loaded = load_versus_actor(path, allow_observed=True)
            self.assertEqual(loaded.model.parameter_count(), model.parameter_count())


if __name__ == "__main__":
    unittest.main()
