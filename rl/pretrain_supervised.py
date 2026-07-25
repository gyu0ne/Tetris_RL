import os
import sys
import time
import torch
import torch.nn as nn
import torch.optim as optim
import numpy as np

sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
from env.tetrio_env import TetrioEnv
from models.afterstate_net import AfterstateValueNet
import tetrio_engine as te

def pretrain_warmstart(num_samples=10000, batch_size=256, epochs=5, lr=1e-3):
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"==================================================")
    print(f" TETR.io Accelerated Warm-Start Pre-Training ")
    print(f" Device: {device} | Target Samples: {num_samples} ")
    print(f"==================================================")

    env = TetrioEnv()
    net = AfterstateValueNet().to(device)

    # 1. Dataset Generation using C++ Fast Heuristic Engine
    print("\n[Step 1/2] Generating High-Quality Expert Dataset using C++ Engine...")
    start_time = time.time()

    boards_list = []
    metas_list = []
    targets_list = []

    samples_collected = 0
    game_id = 0

    while samples_collected < num_samples:
        game_id += 1
        env.reset(seed1=5000 + game_id, seed2=6000 + game_id)
        p1 = env.game.get_p1()

        for _ in range(50): # 50 steps per game
            placements = p1.get_possible_placements()
            if not placements or p1.get_state().is_dead:
                break

            # Select best move via fast C++ Heuristic
            best_action = te.BeamSearchEngine.find_best_move(p1, depth=2, beam_width=8)

            # Record state and heuristic value
            b_tensor, m_vec = env.get_observation(p1, env.game.get_p2())
            info = p1.step(best_action)

            # Standardized Heuristic Target Value
            eval_val = te.BeamSearchEngine.default_heuristic(p1, info) / 10.0

            boards_list.append(b_tensor)
            metas_list.append(m_vec)
            targets_list.append(eval_val)

            samples_collected += 1
            if samples_collected >= num_samples:
                break

    gen_duration = time.time() - start_time
    print(f"  -> Collected {samples_collected} expert samples in {gen_duration:.2f}s ({samples_collected / gen_duration:.0f} samples/sec)!")

    # Convert to PyTorch Tensors
    b_dataset = torch.stack(boards_list)
    m_dataset = torch.stack(metas_list)
    y_dataset = torch.tensor(targets_list, dtype=torch.float32).unsqueeze(1)

    # 2. Supervised Pre-Training (Behavioral Cloning / Value Distillation)
    print("\n[Step 2/2] Pre-Training Neural Network via Value Distillation...")
    optimizer = optim.AdamW(net.parameters(), lr=lr, weight_decay=1e-4)
    loss_fn = nn.MSELoss()

    dataset_size = len(b_dataset)
    num_batches = (dataset_size + batch_size - 1) // batch_size

    net.train()
    pretrain_start = time.time()

    for epoch in range(1, epochs + 1):
        perm = torch.randperm(dataset_size)
        epoch_loss = 0.0

        for b_idx in range(num_batches):
            indices = perm[b_idx * batch_size : (b_idx + 1) * batch_size]
            b_b = b_dataset[indices].to(device)
            m_b = m_dataset[indices].to(device)
            y_b = y_dataset[indices].to(device)

            pred_y = net(b_b, m_b)
            loss = loss_fn(pred_y, y_b)

            optimizer.zero_grad()
            loss.backward()
            optimizer.step()

            epoch_loss += loss.item() * len(indices)

        avg_loss = epoch_loss / dataset_size
        print(f"  Epoch {epoch:02d}/{epochs:02d} | MSE Loss: {avg_loss:.6f}")

    pretrain_duration = time.time() - pretrain_start
    print(f"  -> Neural Network Pre-Training Complete in {pretrain_duration:.2f}s!")

    # Save Pre-Trained Weights
    checkpoint_dir = os.path.join(os.path.dirname(__file__), "../checkpoints")
    os.makedirs(checkpoint_dir, exist_ok=True)
    warmstart_path = os.path.join(checkpoint_dir, "tetrio_sadrl_model.pth")
    torch.save(net.state_dict(), warmstart_path)

    print(f"\n==================================================")
    print(f" SUCCESS: Model Pre-Trained & Saved to: {warmstart_path}")
    print(f" The Neural Agent is now Warm-Started at Expert Level!")
    print(f"==================================================")

if __name__ == "__main__":
    pretrain_warmstart(num_samples=10000, batch_size=256, epochs=5)
