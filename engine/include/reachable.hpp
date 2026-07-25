#ifndef TETRIO_REACHABLE_HPP
#define TETRIO_REACHABLE_HPP

#include "board.hpp"
#include "tetrio_types.hpp"
#include <vector>

namespace tetrio {

class ReachableSearch {
public:
    static std::vector<Placement> find_reachable_placements(
        const Board& board,
        PieceType piece,
        bool is_hold_action = false
    );
};

} // namespace tetrio

#endif // TETRIO_REACHABLE_HPP
