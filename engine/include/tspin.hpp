#ifndef TETRIO_TSPIN_HPP
#define TETRIO_TSPIN_HPP

#include "board.hpp"
#include "tetrio_types.hpp"

namespace tetrio {

class TSpinDetector {
public:
    static SpinType detect(
        const Board& board,
        PieceType piece,
        int x, int y,
        Rotation rot,
        MoveType last_move,
        int kick_index
    );
};

} // namespace tetrio

#endif // TETRIO_TSPIN_HPP
