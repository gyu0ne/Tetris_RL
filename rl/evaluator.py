import os
import sys
import torch
import numpy as np

sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
from env.tetrio_env import TetrioEnv
from models.afterstate_net import AfterstateValueNet
import tetrio_engine as te

def run_evaluation(num_matches=10, beam_depth=3, beam_width=8):
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"==================================================")
    print(f" TETR.io 1v1 Battle Evaluator (SADRL vs Baseline) ")
    print(f"==================================================")

    model_path = os.path.join(os.path.dirname(__file__), "../checkpoints/tetrio_sadrl_model.pth")

    net = AfterstateValueNet().to(device)
    if os.path.exists(model_path):
        net.load_state_dict(torch.load(model_path, map_location=device))
        print(f"Loaded trained checkpoint: {model_path}")
    else:
        print("No trained checkpoint found. Using initialized network.")

    net.eval()
    env = TetrioEnv()

    p1_wins = 0
    p2_wins = 0
    draws = 0

    p1_total_attacks = 0
    p2_total_attacks = 0

    for m in range(1, num_matches + 1):
        env.reset(seed1=3000 + m, seed2=4000 + m)

        steps = 0
        max_steps = 150
        done = False

        while not done and steps < max_steps:
            p1 = env.game.get_p1()
            p2 = env.game.get_p2()

            # P1: SADRL Agent
            def neural_eval(player, attack):
                b_tensor, m_vec = env.get_observation(player, p2)
                with torch.no_grad():
                    v = net(b_tensor.unsqueeze(0).to(device), m_vec.unsqueeze(0).to(device)).item()
                return v + attack.total_attack * 2.0

            action_p1 = te.BeamSearchEngine.find_best_move(
                p1, depth=beam_depth, beam_width=beam_width, eval_fn=neural_eval
            )

            # P2: Baseline Beam Search Heuristic (ColdClear style)
            action_p2 = te.BeamSearchEngine.find_best_move(
                p2, depth=beam_depth, beam_width=beam_width, eval_fn=te.BeamSearchEngine.default_heuristic
            )

            obs, reward, done, info = env.step(action_p1, action_p2)
            steps += 1

            p1_total_attacks += info["info_p1"].total_attack
            p2_total_attacks += info["info_p2"].total_attack

        winner = env.game.get_winner()
        if winner == 1:
            p1_wins += 1
            res_str = "P1 (SADRL Agent) WIN"
        elif winner == 2:
            p2_wins += 1
            res_str = "P2 (Baseline Heuristic) WIN"
        else:
            draws += 1
            res_str = "DRAW"

        print(f"Match {m:02d}/{num_matches:02d} | Result: {res_str:28s} | P1 Attack: {info['info_p1'].total_attack} | P2 Attack: {info['info_p2'].total_attack}")

    win_rate = (p1_wins / num_matches) * 100.0
    print(f"\n==================================================")
    print(f" Evaluation Complete!")
    print(f" SADRL Agent Wins: {p1_wins}/{num_matches} ({win_rate:.1f}%)")
    print(f" Baseline Wins   : {p2_wins}/{num_matches}")
    print(f" Draws           : {draws}/{num_matches}")
    print(f" Avg P1 Attack   : {p1_total_attacks / num_matches:.1f}")
    print(f" Avg P2 Attack   : {p2_total_attacks / num_matches:.1f}")
    print(f"==================================================")

if __name__ == "__main__":
    run_evaluation(num_matches=5, beam_depth=2, beam_width=4)
