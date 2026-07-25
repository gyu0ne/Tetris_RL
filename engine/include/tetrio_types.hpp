#ifndef TETRIO_TYPES_HPP
#define TETRIO_TYPES_HPP

#include <cstdint>
#include <vector>
#include <array>
#include <string>

namespace tetrio {

enum class PieceType : uint8_t {
    NONE = 0,
    I = 1,
    J = 2,
    L = 3,
    O = 4,
    S = 5,
    T = 6,
    Z = 7,
    GARBAGE = 8
};

enum class Rotation : uint8_t {
    SPAWN = 0, // 0
    RIGHT = 1, // R
    REVERSE = 2,// 2
    LEFT = 3   // L
};

enum class SpinType : uint8_t {
    NONE = 0,
    MINI = 1,
    FULL = 2
};

enum class MoveType : uint8_t {
    NORMAL = 0,
    CW = 1,
    CCW = 2,
    ROT180 = 3
};

struct Point {
    int x;
    int y;

    bool operator==(const Point& o) const {
        return x == o.x && y == o.y;
    }
};

struct Placement {
    int x;
    int y;
    Rotation rotation;
    bool hold;
    SpinType spin;
    MoveType move_type;
    int kick_index;
};

struct AttackInfo {
    int lines_cleared = 0;
    PieceType piece_type = PieceType::NONE;
    SpinType spin = SpinType::NONE;
    bool b2b_eligible = false;
    bool all_clear = false;
    int base_attack = 0;
    int b2b_bonus = 0;
    int combo_bonus = 0;
    int total_attack = 0;
    int b2b_level_after = 0;
    int combo_after = 0;
};

struct GarbageEntry {
    int amount;
    int delay_ticks; // Tanking delay
    int hole_column;
};

struct PlayerState {
    int b2b_level = 0;
    int combo = 0;
    int total_attack_sent = 0;
    int total_lines_cleared = 0;
    int total_pieces_placed = 0;
    bool is_dead = false;
};

} // namespace tetrio

#endif // TETRIO_TYPES_HPP
