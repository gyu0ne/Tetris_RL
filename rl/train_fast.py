import os
import sys
import time
import gc
import torch
import torch.nn as nn
import torch.optim as optim
import numpy as np

sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
from env.tetrio_env import TetrioEnv
from models.afterstate_net import AfterstateValueNet
import tetrio_engine as te

def train_robust(episodes=1000, lr=1e-3, save_freq=1, max_pieces_per_ep=100):
    """
    Robust Fast Trainer with Auto-Resume and Memory Safeguards.
    - Never crashes on individual episode errors.
    - Saves checkpoints in real-time.
    - Auto-resumes from previous saved checkpoint.
    """
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"==================================================")
    print(f" TETR.io Robust Fast Trainer (Auto-Resume Enabled)")
    print(f" Device: {device} | Total Target Episodes: {episodes}")
    print(f"==================================================")

    net = AfterstateValueNet().to(device)
    optimizer = optim.AdamW(net.parameters(), lr=lr, weight_decay=1e-4)
    loss_fn = nn.MSELoss()

    checkpoint_dir = os.path.join(os.path.dirname(__file__), "../checkpoints")
    os.makedirs(checkpoint_dir, exist_ok=True)
    latest_ckpt_path = os.path.join(checkpoint_dir, "tetrio_sadrl_model.pth")
    best_ckpt_path = os.path.join(checkpoint_dir, "tetrio_sadrl_best.pth")

    if os.path.exists(latest_ckpt_path):
        try:
            net.load_state_dict(torch.load(latest_ckpt_path, map_location=device))
            print(f"-> Successfully loaded existing checkpoint: {latest_ckpt_path}")
        except Exception as e:
            print(f"-> Warning loading checkpoint: {e}")

    env = TetrioEnv()
    best_return = -float('inf')
    start_total_time = time.time()

    completed_eps = 0

    for episode in range(1, episodes + 1):
        ep_start_time = time.time()
        try:
            board_obs, meta_obs = env.reset(seed1=1000 + episode, seed2=2000 + episode)

            trajectory_boards = []
            trajectory_metas = []
            trajectory_rewards = []

            total_attack_p1 = 0
            total_pieces_p1 = 0
            done = False

            while not done and total_pieces_p1 < max_pieces_per_ep:
                p1 = env.game.get_p1()
                p2 = env.game.get_p2()

                placements_p1 = p1.get_possible_placements()
                if not placements_p1:
                    break

                best_action = te.BeamSearchEngine.find_best_move(p1, depth=2, beam_width=8)

                (next_board, next_meta), reward, done, info = env.step(best_action)

                trajectory_boards.append(board_obs.detach())
                trajectory_metas.append(meta_obs.detach())
                trajectory_rewards.append(reward)

                board_obs, meta_obs = next_board, next_meta
                total_attack_p1 += info["info_p1"].total_attack
                total_pieces_p1 += 1

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

                # Real-Time Checkpoint Save
                torch.save(net.state_dict(), latest_ckpt_path)

                if G > best_return:
                    best_return = G
                    torch.save(net.state_dict(), best_ckpt_path)
                    saved_str = " [NEW BEST!]"
                else:
                    saved_str = ""

                ep_duration = (time.time() - ep_start_time) * 1000.0
                completed_eps += 1

                print(f"Episode {episode:04d}/{episodes:04d} | Time: {ep_duration:5.1f}ms | Pieces: {total_pieces_p1:02d} | Attack: {total_attack_p1:02d} | Return: {G:6.2f} | Loss: {loss.item():.4f}{saved_str}", flush=True)

            # Periodic Garbage Collection every 50 episodes
            if episode % 50 == 0:
                gc.collect()

        except KeyboardInterrupt:
            print("\n[Training Interrupted by User] Saving progress...")
            torch.save(net.state_dict(), latest_ckpt_path)
            break
        except Exception as e:
            print(f"Warning in episode {episode}: {e}. Continuing to next episode...")
            continue

    total_duration = time.time() - start_total_time
    print(f"\n==================================================")
    print(f" Completed {completed_eps} Episodes in {total_duration:.2f}s!")
    print(f" Latest Checkpoint Saved: {latest_ckpt_path}")
    print(f"==================================================")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="TETR.io Robust Trainer")
    parser.add_argument("--episodes", type=int, default=1000)
    parser.add_argument("--lr", type=float, default=1e-3)
    parser.add_argument("--save-freq", type=int, default=1)

    args = parser.parse_args()

    train_robust(episodes=args.episodes, lr=args.lr, save_freq=args.save_freq)
