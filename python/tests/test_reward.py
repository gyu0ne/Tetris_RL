import unittest

import torch
from tetris_rl.training.reward import (
    PotentialConfig,
    state_potential,
    state_potential_components,
    transition_reward,
    transition_reward_details,
)


class PotentialRewardTest(unittest.TestCase):
    def test_component_sum_matches_public_potential_and_reward(self) -> None:
        current = self.state.repeat(2, 1)
        following = self.state.clone()
        following[:, 0] += 2.0
        terminal = torch.tensor([False, False])
        outcomes = torch.zeros(2)

        self.assertTrue(
            torch.allclose(
                state_potential_components(current, self.config).sum(dim=1),
                state_potential(current, self.config),
            )
        )
        reward, components = transition_reward_details(
            current, following, outcomes, terminal, self.config
        )
        self.assertTrue(
            torch.allclose(
                reward, transition_reward(current, following, outcomes, terminal, self.config)
            )
        )
        self.assertTrue(torch.allclose(reward, components.sum(dim=1)))

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
