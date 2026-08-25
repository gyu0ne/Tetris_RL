import argparse
import gzip
import json
import tempfile
import unittest
from hashlib import sha256
from pathlib import Path

import torch
from tetris_rl.features import ACTION_SPACE_ID, FEATURE_NAMES, MECHANICS_STATUS, SCHEMA_VERSION
from tetris_rl.training.dataset import Decision
from tetris_rl.training.imitation import _buffered_shuffle, train


class ImitationTrainingTest(unittest.TestCase):
    def test_buffered_shuffle_is_deterministic_and_keeps_every_decision(self) -> None:
        decisions = [self._decision(seed) for seed in range(12)]

        first = list(_buffered_shuffle(decisions, buffer_size=3, seed=91))
        second = list(_buffered_shuffle(decisions, buffer_size=3, seed=91))

        self.assertEqual(first, second)
        self.assertCountEqual((item.seed for item in first), range(12))
        self.assertNotEqual(first, decisions)

    def test_training_saves_best_epoch_and_stops_after_patience(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._write_dataset(root)
            output = root / "model.pt"
            train(
                argparse.Namespace(
                    manifest=manifest,
                    output=output,
                    epochs=8,
                    min_epochs=2,
                    patience=2,
                    min_improvement=1.0e9,
                    shuffle_buffer=3,
                    batch_decisions=2,
                    learning_rate=3.0e-4,
                    teacher_temperature=1.0,
                    teacher_score_scale=1_000.0,
                    seed=2026,
                    threads=1,
                    allow_observed=True,
                )
            )

            checkpoint = torch.load(output, map_location="cpu", weights_only=True)

            self.assertEqual(checkpoint["training"]["epochs"], 3)
            self.assertEqual(checkpoint["training"]["max_epochs"], 8)
            self.assertTrue(checkpoint["training"]["early_stopped"])
            self.assertEqual(len(checkpoint["training_history"]), 3)
            self.assertIn(checkpoint["training"]["selected_epoch"], (1, 2, 3))

    @staticmethod
    def _decision(seed: int) -> Decision:
        return Decision(
            match_id=f"solo-{seed}",
            seed=seed,
            features=((0.0,) * len(FEATURE_NAMES), (1.0,) * len(FEATURE_NAMES)),
            teacher_scores=(10.0, 0.0),
            chosen_index=0,
        )

    def _write_dataset(self, root: Path) -> Path:
        records = []
        for seed in range(1, 6):
            record = {
                "schema_version": SCHEMA_VERSION,
                "rules_hash": "rules-hash",
                "engine_revision": "test-revision",
                "mechanics_status": MECHANICS_STATUS,
                "action_space": ACTION_SPACE_ID,
                "match_id": f"solo-{seed:016x}",
                "seed": seed,
                "ply": 0,
                "observation_hash": f"observation-{seed}",
                "observation": {},
                "teacher": {},
                "candidates": [
                    {"features": [0] * len(FEATURE_NAMES), "teacher_score": 10, "rank": 0},
                    {"features": [1] * len(FEATURE_NAMES), "teacher_score": 0, "rank": 1},
                ],
                "chosen_index": 0,
                "top_two_margin": 10,
                "terminal_outcome": None,
            }
            records.append(json.dumps(record, separators=(",", ":")) + "\n")
        records_path = root / "records.jsonl.gz"
        with (
            records_path.open("wb") as raw_output,
            gzip.GzipFile(filename="", mode="wb", fileobj=raw_output, mtime=0) as output,
        ):
            output.write("".join(records).encode())
        digest = sha256(records_path.read_bytes()).hexdigest()
        manifest = {
            "schema_version": SCHEMA_VERSION,
            "dataset_id": digest,
            "records_sha256": digest,
            "records_file": records_path.name,
            "rules_hash": "rules-hash",
            "engine_revision": "test-revision",
            "mechanics_status": MECHANICS_STATUS,
            "action_space": ACTION_SPACE_ID,
            "feature_names": FEATURE_NAMES,
        }
        manifest_path = root / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        return manifest_path


if __name__ == "__main__":
    unittest.main()
