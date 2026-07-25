#include "tspin.hpp"

namespace tetrio {

SpinType TSpinDetector::detect(
    const Board& board,
    PieceType piece,
    int x, int y,
    Rotation rot,
    MoveType last_move,
    int kick_index
) {
    if (piece != PieceType::T) return SpinType::NONE;
    if (last_move != MoveType::CW && last_move != MoveType::CCW && last_move != MoveType::ROT180) {
        return SpinType::NONE;
    }

    auto corners = SRSPlus::get_t_corners(rot);
    bool fl = board.is_occupied(x + corners[0].x, y + corners[0].y);
    bool fr = board.is_occupied(x + corners[1].x, y + corners[1].y);
    bool bl = board.is_occupied(x + corners[2].x, y + corners[2].y);
    bool br = board.is_occupied(x + corners[3].x, y + corners[3].y);

    int occupied_corners = (fl ? 1 : 0) + (fr ? 1 : 0) + (bl ? 1 : 0) + (br ? 1 : 0);

    // 3-Corner Rule
    if (occupied_corners < 3) return SpinType::NONE;

    int front_corners = (fl ? 1 : 0) + (fr ? 1 : 0);

    // Both front corners occupied -> Full T-Spin
    if (front_corners == 2) {
        return SpinType::FULL;
    }

    // Only 1 front corner occupied: check kick index 4 (5th kick offset in 0-indexed SRS offset)
    if (kick_index == 4) {
        return SpinType::FULL; // Upgraded T-Spin
    }

    return SpinType::MINI;
}

} // namespace tetrio
