import gzip
import json
import tempfile
import unittest
from hashlib import sha256
from pathlib import Path

from tetris_rl.features import ACTION_SPACE_ID, FEATURE_NAMES, MECHANICS_STATUS, SCHEMA_VERSION
from tetris_rl.training.dataset import (
    DatasetValidationError,
    iter_decisions,
    validate_dataset,
)


class DatasetValidationTest(unittest.TestCase):
    def test_observed_dataset_is_explicit_and_split_stays_match_level(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest_path = self._write_dataset(Path(directory), seed=5)

            with self.assertRaises(DatasetValidationError):
                validate_dataset(manifest_path, allow_observed=False)

            dataset = validate_dataset(manifest_path, allow_observed=True)
            validation = list(iter_decisions(dataset, "validation"))
            training = list(iter_decisions(dataset, "train"))

            self.assertEqual(len(validation), 1)
            self.assertEqual(training, [])
            self.assertEqual(validation[0].chosen_index, 0)

    def test_records_integrity_is_checked_before_parsing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest_path = self._write_dataset(root, seed=1)
            with (root / "records.jsonl.gz").open("ab") as output:
                output.write(b"tamper")

            with self.assertRaisesRegex(DatasetValidationError, "SHA-256"):
                validate_dataset(manifest_path, allow_observed=True)

    def _write_dataset(self, root: Path, *, seed: int) -> Path:
        record = {
            "schema_version": SCHEMA_VERSION,
            "rules_hash": "rules-hash",
            "engine_revision": "test-revision",
            "mechanics_status": MECHANICS_STATUS,
            "action_space": ACTION_SPACE_ID,
            "match_id": f"solo-{seed:016x}",
            "seed": seed,
            "ply": 0,
            "observation_hash": "observation-hash",
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
        records = (json.dumps(record, separators=(",", ":")) + "\n").encode()
        records_path = root / "records.jsonl.gz"
        with (
            records_path.open("wb") as raw_output,
            gzip.GzipFile(filename="", mode="wb", fileobj=raw_output, mtime=0) as output,
        ):
            output.write(records)
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
