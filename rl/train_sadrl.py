import os
import sys
import torch
import torch.nn as nn
import torch.optim as optim
import numpy as np

sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
from env.tetrio_env import TetrioEnv
from models.afterstate_net import AfterstateValueNet

import tetrio_engine as te

def train(episodes=50, beam_depth=2, beam_width=4, lr=1e-4, save_freq=1):
    """
    SADRL Self-Play Training Loop with Frequent Checkpoint Saving.
    - Saves 'checkpoints/tetrio_sadrl_model.pth' after EVERY episode.
    - Saves 'checkpoints/tetrio_sadrl_best.pth' whenever new highest return is achieved.
    - Saves periodic 'checkpoints/tetrio_sadrl_ep{ep}.pth' every save_freq episodes.
    """
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"==================================================")
    print(f" Starting TETR.io Hybrid SADRL Training on {device} ")
    print(f" Episodes: {episodes} | Beam Depth: {beam_depth} | Beam Width: {beam_width} | Save Freq: Every {save_freq} ep")
    print(f"==================================================")

    net = AfterstateValueNet().to(device)

    checkpoint_dir = os.path.join(os.path.dirname(__file__), "../checkpoints")
    os.makedirs(checkpoint_dir, exist_ok=True)
    latest_ckpt_path = os.path.join(checkpoint_dir, "tetrio_sadrl_model.pth")
    best_ckpt_path = os.path.join(checkpoint_dir, "tetrio_sadrl_best.pth")

    # Load existing checkpoint if available
    if os.path.exists(latest_ckpt_path):
        try:
            net.load_state_dict(torch.load(latest_ckpt_path, map_location=device))
            print(f"Loaded existing checkpoint: {latest_ckpt_path}")
        except Exception as e:
            print(f"Could not load checkpoint ({e}), starting fresh weights.")

    optimizer = optim.AdamW(net.parameters(), lr=lr, weight_decay=1e-4)
    loss_fn = nn.MSELoss()

    env = TetrioEnv()

    best_return = -float('inf')

    for episode in range(1, episodes + 1):
        board_obs, meta_obs = env.reset(seed1=1000 + episode, seed2=2000 + episode)

        trajectory_boards = []
        trajectory_metas = []
        trajectory_rewards = []

        total_attack_p1 = 0
        total_pieces_p1 = 0
        done = False

        while not done:
            p1 = env.game.get_p1()
            p2 = env.game.get_p2()

            placements_p1 = p1.get_possible_placements()
            if not placements_p1:
                break

            def neural_eval(player, attack):
                b_tensor, m_vec = env.get_observation(player, p2)
                with torch.no_grad():
                    v = net(b_tensor.unsqueeze(0).to(device), m_vec.unsqueeze(0).to(device)).item()
                return v + attack.total_attack * 2.0

            action_p1 = te.BeamSearchEngine.find_best_move(
                p1, depth=beam_depth, beam_width=beam_width, eval_fn=neural_eval
            )

            (next_board, next_meta), reward, done, info = env.step(action_p1)

            trajectory_boards.append(board_obs)
            trajectory_metas.append(meta_obs)
            trajectory_rewards.append(reward)

            board_obs, meta_obs = next_board, next_meta
            total_attack_p1 += info["info_p1"].total_attack
            total_pieces_p1 += 1

            if total_pieces_p1 >= 100: # Limit episode length for fast iterations
                break

        # Calculate TD Targets
        gamma = 0.99
        returns = []
        G = 0.0
        for r in reversed(trajectory_rewards):
            G = r + gamma * G
            returns.insert(0, G)

        if trajectory_boards:
            b_batch = torch.stack(trajectory_boards).to(device)
            m_batch = torch.stack(trajectory_metas).to(device)
            target_batch = torch.tensor(returns, dtype=torch.float32).unsqueeze(1).to(device)

            net.train()
            values = net(b_batch, m_batch)
            loss = loss_fn(values, target_batch)

            optimizer.zero_grad()
            loss.backward()
            optimizer.step()

            # FREQUENT CHECKPOINT SAVING LOGIC
            # 1. Save latest model after EVERY single episode
            torch.save(net.state_dict(), latest_ckpt_path)

            # 2. Save best checkpoint if new high return
            if G > best_return:
                best_return = G
                torch.save(net.state_dict(), best_ckpt_path)
                saved_best_str = " [NEW BEST!]"
            else:
                saved_best_str = ""

            # 3. Periodic checkpoint every save_freq episodes
            if episode % save_freq == 0:
                ep_ckpt_path = os.path.join(checkpoint_dir, f"tetrio_sadrl_ep{episode}.pth")
                torch.save(net.state_dict(), ep_ckpt_path)

            print(f"Episode {episode:03d}/{episodes:03d} | Pieces: {total_pieces_p1:02d} | Attack: {total_attack_p1:02d} | Return: {G:6.2f} | Loss: {loss.item():.4f} | Saved -> {latest_ckpt_path}{saved_best_str}")

    print(f"\n==================================================")
    print(f" Training Batch Finished! Latest checkpoint: {latest_ckpt_path}")
    print(f" Highest Return Achieved: {best_return:.2f}")
    print(f"==================================================")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="TETR.io SADRL Training")
    parser.add_argument("--episodes", type=int, default=50, help="Number of training episodes")
    parser.add_argument("--depth", type=int, default=2, help="Beam Search Depth")
    parser.add_argument("--width", type=int, default=4, help="Beam Search Width")
    parser.add_argument("--lr", type=float, default=1e-4, help="Learning Rate")
    parser.add_argument("--save-freq", type=int, default=5, help="Periodic Save Frequency")

    args = parser.parse_args()

    train(
        episodes=args.episodes,
        beam_depth=args.depth,
        beam_width=args.width,
        lr=args.lr,
        save_freq=args.save_freq
    )
