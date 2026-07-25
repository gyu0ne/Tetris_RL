import sys
import os
import torch
import numpy as np

# Add build directory to python path for tetrio_engine
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '../../build')))
import tetrio_engine as te

class TetrioEnv:
    def __init__(self, seed1=1337, seed2=4242):
        self.game = te.TetrioVsGame(seed1, seed2)

    def reset(self, seed1=1337, seed2=4242):
        self.game.reset(seed1, seed2)
        return self.get_observation(self.game.get_p1(), self.game.get_p2())

    def get_observation(self, player, opponent):
        # Board tensor (4, 20, 10)
        board = player.get_board()
        opp_board = opponent.get_board()

        board_grid = np.zeros((4, 20, 10), dtype=np.float32)
        rows = board.get_rows()
        opp_rows = opp_board.get_rows()

        for y in range(20):
            for x in range(10):
                if (rows[y] & (1 << x)) != 0:
                    board_grid[0, y, x] = 1.0
                if (opp_rows[y] & (1 << x)) != 0:
                    board_grid[2, y, x] = 1.0

        # Meta Vector (length 64)
        meta = np.zeros((64,), dtype=np.float32)
        p_state = player.get_state()
        o_state = opponent.get_state()

        meta[0] = p_state.b2b_level / 10.0
        meta[1] = p_state.combo / 10.0
        meta[2] = board.get_max_height() / 20.0
        meta[3] = board.count_holes() / 10.0

        meta[4] = o_state.b2b_level / 10.0
        meta[5] = o_state.combo / 10.0
        meta[6] = opp_board.get_max_height() / 20.0
        meta[7] = opp_board.count_holes() / 10.0

        # Current piece one-hot (8-dim)
        meta[8 + int(player.get_current_piece())] = 1.0

        # Hold piece one-hot (8-dim)
        meta[16 + int(player.get_hold_piece())] = 1.0

        # Queue pieces (5 x 8-dim = 40-dim)
        queue = player.get_queue(5)
        for i, piece in enumerate(queue):
            meta[24 + i * 8 + int(piece)] = 1.0

        return torch.from_numpy(board_grid), torch.from_numpy(meta)

    def step(self, action_p1, action_p2=None):
        if action_p2 is None:
            # Self-play baseline move for P2
            p2_placements = self.game.get_p2().get_possible_placements()
            if p2_placements:
                action_p2 = te.BeamSearchEngine.find_best_move(self.game.get_p2(), depth=2, beam_width=8)
            else:
                action_p2 = te.Placement()

        prev_holes_p1 = self.game.get_p1().get_board().count_holes()

        info_p1 = self.game.step_p1(action_p1)
        info_p2 = self.game.step_p2(action_p2)

        p1_dead = self.game.get_p1().get_state().is_dead
        p2_dead = self.game.get_p2().get_state().is_dead

        # Reward Engineering
        reward = 0.0
        if p2_dead and not p1_dead:
            reward += 10.0 # Win reward
        elif p1_dead:
            reward -= 10.0 # Loss penalty
        else:
            reward += info_p1.total_attack * 1.5
            reward += info_p1.b2b_bonus * 0.5

            curr_holes_p1 = self.game.get_p1().get_board().count_holes()
            if curr_holes_p1 < prev_holes_p1:
                reward += (prev_holes_p1 - curr_holes_p1) * 0.8 # Uncovering holes reward

            height = self.game.get_p1().get_board().get_max_height()
            if height > 14:
                reward -= (height - 14) * 0.1 # High stack penalty

        done = p1_dead or p2_dead
        obs = self.get_observation(self.game.get_p1(), self.game.get_p2())

        return obs, reward, done, {"info_p1": info_p1, "info_p2": info_p2}
