import unittest
from pathlib import Path

from tetris_rl.spectator.controller import SpectatorController
from torch import Tensor


class FakeModel:
    @staticmethod
    def parameter_count() -> int:
        return 17


class FakeScorer:
    model = FakeModel()
    metadata = {"engine_revision": "engine-test", "dataset_id": "dataset-test"}

    @staticmethod
    def score(features: Tensor) -> Tensor:
        return features[:, 0]


class FakeBridge:
    def __init__(self, seeds: list[int]) -> None:
        self.seed = seeds[0]
        self.pieces = 0

    def candidates(self) -> tuple[bytes, list[int], list[bool]]:
        if self.pieces >= 2:
            return b"", [0, 0], [True]
        values = [0] * 10 + [1] + [0] * 9
        payload = b"".join(value.to_bytes(4, "little", signed=True) for value in values)
        return payload, [0, 2], [False]

    def step(self, selections: list[int]) -> None:
        if selections != [1]:
            raise AssertionError(f"expected the scorer to select index 1, got {selections}")
        self.pieces += 1

    def snapshot(
        self, index: int
    ) -> tuple[list[int], list[int], str, str | None, list[str], int, bool]:
        if index != 0:
            raise IndexError(index)
        return [0] * 20, [0] * 20, "T", None, ["I", "O", "S", "Z", "J"], self.pieces, False


class SpectatorControllerTest(unittest.TestCase):
    def test_step_uses_real_candidate_scores_and_reports_snapshot(self) -> None:
        controller = SpectatorController(
            Path("model.pt"),
            41,
            allow_observed=True,
            scorer=FakeScorer(),  # type: ignore[arg-type]
            bridge_factory=FakeBridge,
        )

        state = controller.step(1)

        self.assertEqual(state["seed"], 41)
        self.assertEqual(state["pieces_placed"], 1)
        self.assertEqual(state["parameters"], 17)
        self.assertEqual(state["last_decision"]["candidate_count"], 2)
        self.assertEqual(state["last_decision"]["selected_index"], 1)

    def test_reset_rebuilds_the_deterministic_game(self) -> None:
        controller = SpectatorController(
            Path("model.pt"),
            1,
            allow_observed=True,
            scorer=FakeScorer(),  # type: ignore[arg-type]
            bridge_factory=FakeBridge,
        )
        controller.step(1)

        state = controller.reset(99)

        self.assertEqual(state["seed"], 99)
        self.assertEqual(state["pieces_placed"], 0)


if __name__ == "__main__":
    unittest.main()
