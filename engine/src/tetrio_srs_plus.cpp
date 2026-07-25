#include "tetrio_srs_plus.hpp"
#include "board.hpp"

namespace tetrio {

static const std::vector<Point> EMPTY_KICKS = {{0, 0}};

// Kick tables definitions
static const std::vector<Point> JLSTZ_0_TO_R = {{0,0}, {-1,0}, {-1,1}, {0,-2}, {-1,-2}};
static const std::vector<Point> JLSTZ_R_TO_0 = {{0,0}, {1,0}, {1,-1}, {0,2}, {1,2}};
static const std::vector<Point> JLSTZ_R_TO_2 = {{0,0}, {1,0}, {1,-1}, {0,2}, {1,2}};
static const std::vector<Point> JLSTZ_2_TO_R = {{0,0}, {-1,0}, {-1,1}, {0,-2}, {-1,-2}};
static const std::vector<Point> JLSTZ_2_TO_L = {{0,0}, {1,0}, {1,1}, {0,-2}, {1,-2}};
static const std::vector<Point> JLSTZ_L_TO_2 = {{0,0}, {-1,0}, {-1,-1}, {0,2}, {-1,2}};
static const std::vector<Point> JLSTZ_L_TO_0 = {{0,0}, {-1,0}, {-1,-1}, {0,2}, {-1,2}};
static const std::vector<Point> JLSTZ_0_TO_L = {{0,0}, {1,0}, {1,1}, {0,-2}, {1,-2}};

// I-Piece SRS+ Kick Tables
static const std::vector<Point> I_0_TO_R = {{0,0}, {-2,0}, {1,0}, {-2,-1}, {1,2}};
static const std::vector<Point> I_R_TO_0 = {{0,0}, {2,0}, {-1,0}, {2,1}, {-1,-2}};
static const std::vector<Point> I_R_TO_2 = {{0,0}, {-1,0}, {2,0}, {-1,2}, {2,-1}};
static const std::vector<Point> I_2_TO_R = {{0,0}, {1,0}, {-2,0}, {1,-2}, {-2,1}};
static const std::vector<Point> I_2_TO_L = {{0,0}, {2,0}, {-1,0}, {2,1}, {-1,-2}};
static const std::vector<Point> I_L_TO_2 = {{0,0}, {-2,0}, {1,0}, {-2,-1}, {1,2}};
static const std::vector<Point> I_L_TO_0 = {{0,0}, {1,0}, {-2,0}, {1,-2}, {-2,1}};
static const std::vector<Point> I_0_TO_L = {{0,0}, {-1,0}, {2,0}, {-1,2}, {2,-1}};

// 180 Rotation Kick Tables (JLSTZ)
static const std::vector<Point> JLSTZ_0_TO_2 = {{0,0}, {0,1}, {1,1}, {-1,1}, {1,0}, {-1,0}, {0,-1}};
static const std::vector<Point> JLSTZ_2_TO_0 = {{0,0}, {0,-1}, {-1,-1}, {1,-1}, {-1,0}, {1,0}, {0,1}};
static const std::vector<Point> JLSTZ_R_TO_L = {{0,0}, {1,0}, {1,2}, {1,1}, {0,2}, {0,1}, {1,-1}};
static const std::vector<Point> JLSTZ_L_TO_R = {{0,0}, {-1,0}, {-1,2}, {-1,1}, {0,2}, {0,1}, {-1,-1}};

// 180 Rotation Kick Tables (I-Piece)
static const std::vector<Point> I_180_0_TO_2 = {{0,0}, {-1,0}, {-2,0}, {1,0}, {2,0}, {0,1}};
static const std::vector<Point> I_180_2_TO_0 = {{0,0}, {1,0}, {2,0}, {-1,0}, {-2,0}, {0,-1}};
static const std::vector<Point> I_180_R_TO_L = {{0,0}, {0,1}, {0,2}, {0,-1}, {0,-2}, {-1,0}};
static const std::vector<Point> I_180_L_TO_R = {{0,0}, {0,1}, {0,2}, {0,-1}, {0,-2}, {1,0}};

std::array<Point, 4> SRSPlus::get_shape(PieceType piece, Rotation rot) {
    switch (piece) {
        case PieceType::I:
            switch (rot) {
                case Rotation::SPAWN:   return {{{-1, 0}, {0, 0}, {1, 0}, {2, 0}}};
                case Rotation::RIGHT:   return {{{1, 1}, {1, 0}, {1, -1}, {1, -2}}};
                case Rotation::REVERSE: return {{{-1, -1}, {0, -1}, {1, -1}, {2, -1}}};
                case Rotation::LEFT:    return {{{0, 1}, {0, 0}, {0, -1}, {0, -2}}};
            }
            break;

        case PieceType::J:
            switch (rot) {
                case Rotation::SPAWN:   return {{{-1, 1}, {-1, 0}, {0, 0}, {1, 0}}};
                case Rotation::RIGHT:   return {{{0, 1}, {1, 1}, {0, 0}, {0, -1}}};
                case Rotation::REVERSE: return {{{-1, 0}, {0, 0}, {1, 0}, {1, -1}}};
                case Rotation::LEFT:    return {{{0, 1}, {0, 0}, {0, -1}, {-1, -1}}};
            }
            break;

        case PieceType::L:
            switch (rot) {
                case Rotation::SPAWN:   return {{{1, 1}, {-1, 0}, {0, 0}, {1, 0}}};
                case Rotation::RIGHT:   return {{{0, 1}, {0, 0}, {0, -1}, {1, -1}}};
                case Rotation::REVERSE: return {{{-1, 0}, {0, 0}, {1, 0}, {-1, -1}}};
                case Rotation::LEFT:    return {{{-1, 1}, {0, 1}, {0, 0}, {0, -1}}};
            }
            break;

        case PieceType::O:
            return {{{0, 1}, {1, 1}, {0, 0}, {1, 0}}};

        case PieceType::S:
            switch (rot) {
                case Rotation::SPAWN:   return {{{0, 1}, {1, 1}, {-1, 0}, {0, 0}}};
                case Rotation::RIGHT:   return {{{0, 1}, {0, 0}, {1, 0}, {1, -1}}};
                case Rotation::REVERSE: return {{{0, 0}, {1, 0}, {-1, -1}, {0, -1}}};
                case Rotation::LEFT:    return {{{-1, 1}, {-1, 0}, {0, 0}, {0, -1}}};
            }
            break;

        case PieceType::T:
            switch (rot) {
                case Rotation::SPAWN:   return {{{0, 1}, {-1, 0}, {0, 0}, {1, 0}}};
                case Rotation::RIGHT:   return {{{0, 1}, {0, 0}, {1, 0}, {0, -1}}};
                case Rotation::REVERSE: return {{{-1, 0}, {0, 0}, {1, 0}, {0, -1}}};
                case Rotation::LEFT:    return {{{0, 1}, {-1, 0}, {0, 0}, {0, -1}}};
            }
            break;

        case PieceType::Z:
            switch (rot) {
                case Rotation::SPAWN:   return {{{-1, 1}, {0, 1}, {0, 0}, {1, 0}}};
                case Rotation::RIGHT:   return {{{1, 1}, {0, 0}, {1, 0}, {0, -1}}};
                case Rotation::REVERSE: return {{{-1, 0}, {0, 0}, {0, -1}, {1, -1}}};
                case Rotation::LEFT:    return {{{0, 1}, {-1, 0}, {0, 0}, {-1, -1}}};
            }
            break;

        default:
            break;
    }
    return {{{0,0}, {0,0}, {0,0}, {0,0}}};
}

const std::vector<Point>& SRSPlus::get_kicks(PieceType piece, Rotation from_rot, Rotation to_rot) {
    if (piece == PieceType::O) return EMPTY_KICKS;

    // 180 Rotations
    if ((from_rot == Rotation::SPAWN && to_rot == Rotation::REVERSE)) {
        return (piece == PieceType::I) ? I_180_0_TO_2 : JLSTZ_0_TO_2;
    }
    if ((from_rot == Rotation::REVERSE && to_rot == Rotation::SPAWN)) {
        return (piece == PieceType::I) ? I_180_2_TO_0 : JLSTZ_2_TO_0;
    }
    if ((from_rot == Rotation::RIGHT && to_rot == Rotation::LEFT)) {
        return (piece == PieceType::I) ? I_180_R_TO_L : JLSTZ_R_TO_L;
    }
    if ((from_rot == Rotation::LEFT && to_rot == Rotation::RIGHT)) {
        return (piece == PieceType::I) ? I_180_L_TO_R : JLSTZ_L_TO_R;
    }

    // 90 Rotations
    if (piece == PieceType::I) {
        if (from_rot == Rotation::SPAWN && to_rot == Rotation::RIGHT) return I_0_TO_R;
        if (from_rot == Rotation::RIGHT && to_rot == Rotation::SPAWN) return I_R_TO_0;
        if (from_rot == Rotation::RIGHT && to_rot == Rotation::REVERSE) return I_R_TO_2;
        if (from_rot == Rotation::REVERSE && to_rot == Rotation::RIGHT) return I_2_TO_R;
        if (from_rot == Rotation::REVERSE && to_rot == Rotation::LEFT) return I_2_TO_L;
        if (from_rot == Rotation::LEFT && to_rot == Rotation::REVERSE) return I_L_TO_2;
        if (from_rot == Rotation::LEFT && to_rot == Rotation::SPAWN) return I_L_TO_0;
        if (from_rot == Rotation::SPAWN && to_rot == Rotation::LEFT) return I_0_TO_L;
    } else {
        if (from_rot == Rotation::SPAWN && to_rot == Rotation::RIGHT) return JLSTZ_0_TO_R;
        if (from_rot == Rotation::RIGHT && to_rot == Rotation::SPAWN) return JLSTZ_R_TO_0;
        if (from_rot == Rotation::RIGHT && to_rot == Rotation::REVERSE) return JLSTZ_R_TO_2;
        if (from_rot == Rotation::REVERSE && to_rot == Rotation::RIGHT) return JLSTZ_2_TO_R;
        if (from_rot == Rotation::REVERSE && to_rot == Rotation::LEFT) return JLSTZ_2_TO_L;
        if (from_rot == Rotation::LEFT && to_rot == Rotation::REVERSE) return JLSTZ_L_TO_2;
        if (from_rot == Rotation::LEFT && to_rot == Rotation::SPAWN) return JLSTZ_L_TO_0;
        if (from_rot == Rotation::SPAWN && to_rot == Rotation::LEFT) return JLSTZ_0_TO_L;
    }

    return EMPTY_KICKS;
}

std::array<Point, 4> SRSPlus::get_t_corners(Rotation rot) {
    switch (rot) {
        case Rotation::SPAWN:   return {{{-1, 1}, {1, 1}, {-1, -1}, {1, -1}}};
        case Rotation::RIGHT:   return {{{1, 1}, {1, -1}, {-1, 1}, {-1, -1}}};
        case Rotation::REVERSE: return {{{1, -1}, {-1, -1}, {1, 1}, {-1, 1}}};
        case Rotation::LEFT:    return {{{-1, -1}, {-1, 1}, {1, -1}, {1, 1}}};
    }
    return {{{-1, 1}, {1, 1}, {-1, -1}, {1, -1}}};
}

RotateResult SRSPlus::try_rotate(const Board& board, PieceType piece, Rotation from_rot, Rotation to_rot, int x, int y) {
    const auto& kicks = get_kicks(piece, from_rot, to_rot);
    for (size_t k = 0; k < kicks.size(); ++k) {
        int nx = x + kicks[k].x;
        int ny = y + kicks[k].y;
        if (board.can_place(piece, to_rot, nx, ny)) {
            return {true, nx, ny, to_rot, static_cast<int>(k)};
        }
    }
    return {false, x, y, from_rot, 0};
}

} // namespace tetrio
