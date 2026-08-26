import argparse
import gzip
import json
import subprocess
import sys
import tempfile
import unittest
from hashlib import sha256
from pathlib import Path
from unittest import mock

import tetris_rl.training.imitation as imitation
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

    def test_training_resumes_exactly_from_last_completed_epoch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._write_dataset(root)
            resumed_output = root / "resumed.pt"
            resumed_args = self._training_args(manifest, resumed_output, resume=True)
            original_run_epoch = imitation._run_epoch
            calls = 0

            def interrupt_second_epoch(*args: object, **kwargs: object):  # type: ignore[no-untyped-def]
                nonlocal calls
                calls += 1
                if calls == 3:
                    raise RuntimeError("simulated interruption")
                return original_run_epoch(*args, **kwargs)

            with (
                mock.patch.object(imitation, "_run_epoch", side_effect=interrupt_second_epoch),
                self.assertRaisesRegex(RuntimeError, "simulated interruption"),
            ):
                train(resumed_args)

            progress = root / "resumed.progress.pt"
            progress_payload = torch.load(progress, map_location="cpu", weights_only=True)
            self.assertEqual(progress_payload["completed_epochs"], 1)
            self.assertFalse(resumed_output.exists())

            train(resumed_args)
            self.assertFalse(progress.exists())
            resumed = torch.load(resumed_output, map_location="cpu", weights_only=True)
            self.assertEqual([item["epoch"] for item in resumed["training_history"]], [1, 2, 3, 4])

            uninterrupted_output = root / "uninterrupted.pt"
            train(self._training_args(manifest, uninterrupted_output, resume=False))
            uninterrupted = torch.load(
                uninterrupted_output,
                map_location="cpu",
                weights_only=True,
            )
            for name, tensor in resumed["model_state"].items():
                self.assertTrue(torch.equal(tensor, uninterrupted["model_state"][name]))

    def test_parallel_multiseed_reuses_completed_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._write_dataset(root)
            output = root / "candidates"
            command = [
                sys.executable,
                "-m",
                "tetris_rl.training.multiseed",
                "--manifest",
                str(manifest),
                "--output-dir",
                str(output),
                "--seeds",
                "31",
                "32",
                "33",
                "--epochs",
                "2",
                "--min-epochs",
                "2",
                "--patience",
                "2",
                "--min-improvement",
                "1000000000",
                "--shuffle-buffer",
                "3",
                "--batch-decisions",
                "2",
                "--threads",
                "1",
                "--workers",
                "2",
                "--resume",
                "--allow-observed",
            ]
            first = subprocess.run(command, check=True, capture_output=True, text=True)
            self.assertIn('"workers": 2', first.stdout)
            self.assertEqual(len(list(output.glob("seed-*.pt"))), 3)

            second = subprocess.run(command, check=True, capture_output=True, text=True)
            self.assertEqual(second.stdout.count('"event": "completed_candidate_reused"'), 3)

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

    @staticmethod
    def _training_args(
        manifest: Path,
        output: Path,
        *,
        resume: bool,
    ) -> argparse.Namespace:
        return argparse.Namespace(
            manifest=manifest,
            output=output,
            epochs=4,
            min_epochs=4,
            patience=10,
            min_improvement=1.0e9,
            shuffle_buffer=3,
            batch_decisions=2,
            learning_rate=3.0e-4,
            teacher_temperature=1.0,
            teacher_score_scale=1_000.0,
            seed=2026,
            threads=1,
            resume=resume,
            allow_observed=True,
        )


if __name__ == "__main__":
    unittest.main()
