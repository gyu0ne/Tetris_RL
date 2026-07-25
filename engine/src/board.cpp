#include "board.hpp"
#include <algorithm>
#include <cmath>

namespace tetrio {

Board::Board() {
    reset();
}

void Board::reset() {
    rows_.fill(0);
    for (int y = 0; y < HEIGHT; ++y) {
        cells_[y].fill(PieceType::NONE);
    }
}

bool Board::is_occupied(int x, int y) const {
    if (x < 0 || x >= WIDTH || y < 0) return true;
    if (y >= HEIGHT) return false;
    return (rows_[y] & (1 << x)) != 0;
}

PieceType Board::get_cell(int x, int y) const {
    if (x < 0 || x >= WIDTH || y < 0 || y >= HEIGHT) return PieceType::NONE;
    if ((rows_[y] & (1 << x)) == 0) return PieceType::NONE;
    return cells_[y][x];
}

bool Board::can_place(PieceType piece, Rotation rot, int x, int y) const {
    if (piece == PieceType::NONE) return false;
    auto shape = SRSPlus::get_shape(piece, rot);

    for (const auto& pt : shape) {
        int px = x + pt.x;
        int py = y + pt.y;

        if (px < 0 || px >= WIDTH || py < 0) return false;
        if (py < HEIGHT && (rows_[py] & (1 << px))) return false;
    }
    return true;
}

void Board::place_piece(PieceType piece, Rotation rot, int x, int y) {
    if (piece == PieceType::NONE) return;
    auto shape = SRSPlus::get_shape(piece, rot);

    for (const auto& pt : shape) {
        int px = x + pt.x;
        int py = y + pt.y;

        if (px >= 0 && px < WIDTH && py >= 0 && py < HEIGHT) {
            rows_[py] |= (1 << px);
            cells_[py][px] = piece;
        }
    }
}

int Board::clear_lines() {
    int cleared = 0;
    int write_y = 0;

    for (int read_y = 0; read_y < HEIGHT; ++read_y) {
        if (rows_[read_y] == FULL_ROW_MASK) {
            cleared++;
        } else {
            if (write_y != read_y) {
                rows_[write_y] = rows_[read_y];
                cells_[write_y] = cells_[read_y];
            }
            write_y++;
        }
    }

    while (write_y < HEIGHT) {
        rows_[write_y] = 0;
        cells_[write_y].fill(PieceType::NONE);
        write_y++;
    }

    return cleared;
}

void Board::add_garbage(int lines, int hole_col) {
    if (lines <= 0 || hole_col < 0 || hole_col >= WIDTH) return;

    // Shift existing rows and cells up
    for (int y = HEIGHT - 1; y >= lines; --y) {
        rows_[y] = rows_[y - lines];
        cells_[y] = cells_[y - lines];
    }

    // Insert garbage rows at bottom
    uint16_t garbage_row = FULL_ROW_MASK & ~(1 << hole_col);
    for (int y = 0; y < lines && y < HEIGHT; ++y) {
        rows_[y] = garbage_row;
        for (int x = 0; x < WIDTH; ++x) {
            cells_[y][x] = (x == hole_col) ? PieceType::NONE : PieceType::GARBAGE;
        }
    }
}

bool Board::is_top_out() const {
    for (int y = VISIBLE_HEIGHT; y < HEIGHT; ++y) {
        if (rows_[y] != 0) return true;
    }
    return false;
}

int Board::get_column_height(int col) const {
    if (col < 0 || col >= WIDTH) return 0;
    for (int y = HEIGHT - 1; y >= 0; --y) {
        if (rows_[y] & (1 << col)) return y + 1;
    }
    return 0;
}

int Board::get_max_height() const {
    int max_h = 0;
    for (int col = 0; col < WIDTH; ++col) {
        max_h = std::max(max_h, get_column_height(col));
    }
    return max_h;
}

int Board::count_holes() const {
    int holes = 0;
    for (int col = 0; col < WIDTH; ++col) {
        bool block_found = false;
        for (int y = HEIGHT - 1; y >= 0; --y) {
            if (rows_[y] & (1 << col)) {
                block_found = true;
            } else if (block_found) {
                holes++;
            }
        }
    }
    return holes;
}

int Board::get_bumpiness() const {
    int bumpiness = 0;
    int prev_h = get_column_height(0);
    for (int col = 1; col < WIDTH; ++col) {
        int curr_h = get_column_height(col);
        bumpiness += std::abs(curr_h - prev_h);
        prev_h = curr_h;
    }
    return bumpiness;
}

} // namespace tetrio
