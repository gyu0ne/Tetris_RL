import tempfile
import unittest
from pathlib import Path

import torch
from tetris_rl.features import FEATURE_NAMES, MECHANICS_STATUS, SCHEMA_VERSION
from tetris_rl.models import AfterstateScorer, load_scorer


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


if __name__ == "__main__":
    unittest.main()
