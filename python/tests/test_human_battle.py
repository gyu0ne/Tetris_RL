from __future__ import annotations

import unittest
from pathlib import Path
from types import SimpleNamespace

import numpy as np
import torch
from tetris_rl.envs.versus import CANDIDATE_FEATURE_COUNT
from tetris_rl.human_battle.controller import HumanBattleController
from torch import nn


class FakeActorModel(nn.Module):
    def __init__(self) -> None:
        super().__init__()
        self.weight = nn.Parameter(torch.ones(1))

    def actor_logits(self, features: torch.Tensor) -> torch.Tensor:
        return features[:, 0]


class FakeBridge:
    def __init__(self, seed: int, cadence: int) -> None:
        self.seed = seed
        self.cadence = cadence
        self.frame = 0
        self.last_selection = -1

    def bot_candidates(self) -> tuple[bytes, bool, bool]:
        features = np.zeros((2, CANDIDATE_FEATURE_COUNT), dtype="<i4")
        features[:, 0] = [1, 3]
        return features.tobytes(), self.frame == 0, False

    def step(self, edges: list[tuple[str, str]], selection: int) -> None:
        self.last_selection = selection
        self.frame += 1

    def snapshot(self) -> tuple[object, ...]:
        player = (
            [0] * 20,
            [0] * 20,
            ("T", [(3, 19), (4, 19), (5, 19), (4, 20)]),
            None,
            ["I", "O", "S"],
            0,
            0,
            0,
            0,
            0,
            0,
        )
        return self.frame, "ongoing", self.cadence, self.cadence, player, player


class HumanBattleControllerTest(unittest.TestCase):
    def new_controller(self) -> HumanBattleController:
        actor = SimpleNamespace(model=FakeActorModel(), metadata={"update": 428})
        return HumanBattleController(
            Path("fake.pt"),
            5,
            allow_observed=True,
            actor=actor,
            bridge_factory=FakeBridge,
        )

    def test_frame_scores_due_candidates_and_uses_argmax(self) -> None:
        controller = self.new_controller()
        state = controller.step([{"button": "hard_drop", "kind": "press"}])

        self.assertEqual(state["frame"], 1)
        self.assertEqual(state["last_decision"]["candidate_count"], 2)
        self.assertEqual(state["last_decision"]["selected_index"], 1)
        self.assertEqual(state["training_update"], 428)

    def test_reset_rebuilds_bridge_with_new_seed(self) -> None:
        controller = self.new_controller()
        state = controller.reset(17)
        self.assertEqual(state["seed"], 17)
        self.assertEqual(state["frame"], 0)

    def test_unknown_input_is_rejected(self) -> None:
        controller = self.new_controller()
        with self.assertRaisesRegex(ValueError, "unknown input edge"):
            controller.step([{"button": "teleport", "kind": "press"}])


if __name__ == "__main__":
    unittest.main()
