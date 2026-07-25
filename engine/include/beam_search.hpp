#ifndef TETRIO_BEAM_SEARCH_HPP
#define TETRIO_BEAM_SEARCH_HPP

#include "engine.hpp"
#include <functional>

namespace tetrio {

using EvaluatorFn = std::function<float(const TetrioPlayer& player, const AttackInfo& attack)>;

class BeamSearchEngine {
public:
    static Placement find_best_move(
        const TetrioPlayer& player,
        int depth = 4,
        int beam_width = 16,
        EvaluatorFn eval_fn = nullptr
    );

    // Default heuristic evaluator (for baseline comparison)
    static float default_heuristic(const TetrioPlayer& player, const AttackInfo& attack);
};

} // namespace tetrio

#endif // TETRIO_BEAM_SEARCH_HPP
