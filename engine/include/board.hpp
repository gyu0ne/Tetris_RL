#ifndef TETRIO_BOARD_HPP
#define TETRIO_BOARD_HPP

#include "tetrio_types.hpp"
#include "tetrio_srs_plus.hpp"
#include <array>
#include <cstdint>
#include <vector>

namespace tetrio {

class Board {
public:
    static constexpr int WIDTH = 10;
    static constexpr int HEIGHT = 40;
    static constexpr int VISIBLE_HEIGHT = 20;
    static constexpr uint16_t FULL_ROW_MASK = (1 << WIDTH) - 1; // 0x3FF

    Board();
    void reset();

    bool is_occupied(int x, int y) const;
    PieceType get_cell(int x, int y) const;

    bool can_place(PieceType piece, Rotation rot, int x, int y) const;
    void place_piece(PieceType piece, Rotation rot, int x, int y);

    int clear_lines();
    void add_garbage(int lines, int hole_col);

    bool is_top_out() const;

    int get_column_height(int col) const;
    int get_max_height() const;
    int count_holes() const;
    int get_bumpiness() const;

    const std::array<uint16_t, HEIGHT>& get_rows() const { return rows_; }
    uint16_t get_row(int y) const { return (y >= 0 && y < HEIGHT) ? rows_[y] : 0; }

private:
    std::array<uint16_t, HEIGHT> rows_;
    std::array<std::array<PieceType, WIDTH>, HEIGHT> cells_;
};

} // namespace tetrio

#endif // TETRIO_BOARD_HPP
