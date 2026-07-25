#include "beam_search.hpp"
#include <algorithm>
#include <vector>

namespace tetrio {

struct BeamNode {
    TetrioPlayer player;
    Placement first_action;
    float accumulated_score;
    AttackInfo last_attack;
};

float BeamSearchEngine::default_heuristic(const TetrioPlayer& player, const AttackInfo& attack) {
    const auto& board = player.get_board();
    const auto& state = player.get_state();

    if (state.is_dead) return -999999.0f;

    float score = 0.0f;

    // 1. Height penalty
    int max_h = board.get_max_height();
    score -= (max_h * max_h) * 0.5f;

    // 2. Holes penalty
    int holes = board.count_holes();
    score -= holes * 15.0f;

    // 3. Bumpiness penalty
    int bumpiness = board.get_bumpiness();
    score -= bumpiness * 2.0f;

    // 4. Attack reward
    score += attack.total_attack * 20.0f;

    // 5. B2B Level bonus
    score += state.b2b_level * 5.0f;

    // 6. T-Spin / All-Clear bonus
    if (attack.spin == SpinType::FULL) score += 15.0f;
    if (attack.all_clear) score += 50.0f;

    return score;
}

Placement BeamSearchEngine::find_best_move(
    const TetrioPlayer& player,
    int depth,
    int beam_width,
    EvaluatorFn eval_fn
) {
    if (!eval_fn) {
        eval_fn = default_heuristic;
    }

    auto root_placements = const_cast<TetrioPlayer&>(player).get_possible_placements();
    if (root_placements.empty()) {
        return Placement{0, 0, Rotation::SPAWN, false, SpinType::NONE, MoveType::NORMAL, 0};
    }

    std::vector<BeamNode> current_beam;
    current_beam.reserve(root_placements.size());

    // Initialize root depth (depth 1)
    for (const auto& p : root_placements) {
        TetrioPlayer next_p = player;
        AttackInfo atk = next_p.step(p);
        float score = eval_fn(next_p, atk);

        current_beam.push_back({next_p, p, score, atk});
    }

    // Keep top beam_width nodes
    std::sort(current_beam.begin(), current_beam.end(), [](const BeamNode& a, const BeamNode& b) {
        return a.accumulated_score > b.accumulated_score;
    });

    if (static_cast<int>(current_beam.size()) > beam_width) {
        current_beam.resize(beam_width);
    }

    // Expand search to deeper levels
    for (int d = 2; d <= depth; ++d) {
        std::vector<BeamNode> next_beam;

        for (const auto& node : current_beam) {
            if (node.player.get_state().is_dead) continue;

            auto placements = const_cast<TetrioPlayer&>(node.player).get_possible_placements();
            for (const auto& p : placements) {
                TetrioPlayer child_player = node.player;
                AttackInfo atk = child_player.step(p);
                float step_score = eval_fn(child_player, atk);
                float total_score = node.accumulated_score * 0.9f + step_score;

                next_beam.push_back({child_player, node.first_action, total_score, atk});
            }
        }

        if (next_beam.empty()) break;

        std::sort(next_beam.begin(), next_beam.end(), [](const BeamNode& a, const BeamNode& b) {
            return a.accumulated_score > b.accumulated_score;
        });

        if (static_cast<int>(next_beam.size()) > beam_width) {
            next_beam.resize(beam_width);
        }

        current_beam = std::move(next_beam);
    }

    return current_beam.front().first_action;
}

} // namespace tetrio
