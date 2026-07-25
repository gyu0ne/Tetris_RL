#ifndef TETRIO_SRS_PLUS_HPP
#define TETRIO_SRS_PLUS_HPP

#include "tetrio_types.hpp"
#include <array>
#include <vector>

namespace tetrio {

class Board; // Forward declaration

struct RotateResult {
    bool success = false;
    int new_x = 0;
    int new_y = 0;
    Rotation new_rot = Rotation::SPAWN;
    int kick_index = 0;
};

class SRSPlus {
public:
    static std::array<Point, 4> get_shape(PieceType piece, Rotation rot);
    static const std::vector<Point>& get_kicks(PieceType piece, Rotation from_rot, Rotation to_rot);
    static std::array<Point, 4> get_t_corners(Rotation rot);

    static RotateResult try_rotate(
        const Board& board,
        PieceType piece,
        Rotation from_rot,
        Rotation to_rot,
        int x, int y
    );
};

} // namespace tetrio

#endif // TETRIO_SRS_PLUS_HPP
