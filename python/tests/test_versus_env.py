import unittest

from tetris_rl.envs import VersusVectorEnv


class FakeVersusBridge:
    def __init__(self) -> None:
        self.stepped = False

    def candidates(self):
        features = (b"\x00\x00\x00\x00" * 20) * 2
        states = (b"\x00\x00\x00\x00" * 12) * 2
        return features, states, [0, 1, 2], [False, False], [0, 0]

    def step(self, selections):
        self.stepped = selections == [0, 0]

    def reset_done(self, seeds):
        self.reset_seeds = seeds

    def match_count(self):
        return 1


class VersusVectorEnvTest(unittest.TestCase):
    def test_decodes_two_player_variable_candidate_batch(self) -> None:
        bridge = FakeVersusBridge()
        env = VersusVectorEnv([1], bridge=bridge)

        observation = env.observe()
        self.assertEqual(tuple(observation.candidate_features.shape), (2, 20))
        self.assertEqual(tuple(observation.state_features.shape), (2, 12))
        env.step([0, 0])
        self.assertTrue(bridge.stepped)


if __name__ == "__main__":
    unittest.main()
