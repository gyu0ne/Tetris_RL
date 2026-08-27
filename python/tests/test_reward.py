import unittest

import torch
from tetris_rl.training.reward import PotentialConfig, state_potential, transition_reward


class PotentialRewardTest(unittest.TestCase):
    def setUp(self) -> None:
        self.config = PotentialConfig()
        self.state = torch.tensor([[8, 14, 2, 6, 3, 11, 1, 5, 4, 1, 7, 2]], dtype=torch.float32)

    def test_player_swap_negates_potential(self) -> None:
        swapped = self.state.reshape(1, 6, 2).flip(-1).reshape(1, 12)
        one = state_potential(self.state, self.config)
        two = state_potential(swapped, self.config)

        torch.testing.assert_close(one, -two)
        self.assertLessEqual(abs(float(one.item())), 1.0)

    def test_terminal_reward_is_zero_sum(self) -> None:
        swapped = self.state.reshape(1, 6, 2).flip(-1).reshape(1, 12)
        current = torch.cat((self.state, swapped), dim=0)
        next_state = torch.zeros_like(current)
        outcome = torch.tensor([1.0, -1.0])
        terminal = torch.tensor([True, True])

        reward = transition_reward(current, next_state, outcome, terminal, self.config)

        torch.testing.assert_close(reward[0], -reward[1])

    def test_discounted_shaping_telescopes_to_fixed_boundary(self) -> None:
        states = torch.cat((self.state, self.state * 0.5, self.state * 0.25), dim=0)
        phi = state_potential(states, self.config)
        shaping = [self.config.gamma * phi[index + 1] - phi[index] for index in range(len(phi) - 1)]
        shaping.append(-phi[-1])
        discounted = sum((self.config.gamma**index) * value for index, value in enumerate(shaping))

        torch.testing.assert_close(discounted, -phi[0])


if __name__ == "__main__":
    unittest.main()
