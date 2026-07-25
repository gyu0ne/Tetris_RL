#include "reachable.hpp"
#include "tspin.hpp"
#include <queue>
#include <array>
#include <unordered_set>

namespace tetrio {

struct SearchState {
    int x;
    int y;
    Rotation rot;
    MoveType last_move;
    int kick_index;
};

static Rotation rotate_cw(Rotation r) {
    return static_cast<Rotation>((static_cast<int>(r) + 1) % 4);
}

static Rotation rotate_ccw(Rotation r) {
    return static_cast<Rotation>((static_cast<int>(r) + 3) % 4);
}

static Rotation rotate_180(Rotation r) {
    return static_cast<Rotation>((static_cast<int>(r) + 2) % 4);
}

std::vector<Placement> ReachableSearch::find_reachable_placements(
    const Board& board,
    PieceType piece,
    bool is_hold_action
) {
    std::vector<Placement> results;
    if (piece == PieceType::NONE) return results;

    // Default spawn coordinates
    int spawn_x = 3;
    int spawn_y = 19;
    if (piece == PieceType::O) spawn_x = 4;

    // Check if initial spawn is valid
    if (!board.can_place(piece, Rotation::SPAWN, spawn_x, spawn_y)) {
        // Try spawn 1 tile higher if top space permits
        spawn_y = 20;
        if (!board.can_place(piece, Rotation::SPAWN, spawn_x, spawn_y)) {
            return results; // Block out / cannot spawn
        }
    }

    bool visited[Board::HEIGHT][Board::WIDTH][4] = {false};
    bool landed_added[Board::HEIGHT][Board::WIDTH][4] = {false};

    std::queue<SearchState> q;
    q.push({spawn_x, spawn_y, Rotation::SPAWN, MoveType::NORMAL, 0});
    visited[spawn_y][spawn_x][static_cast<int>(Rotation::SPAWN)] = true;

    while (!q.empty()) {
        auto curr = q.front();
        q.pop();

        int x = curr.x;
        int y = curr.y;
        Rotation r = curr.rot;

        // Find hard drop land position
        int drop_y = y;
        while (board.can_place(piece, r, x, drop_y - 1)) {
            drop_y--;
        }

        int r_idx = static_cast<int>(r);
        if (!landed_added[drop_y][x][r_idx]) {
            landed_added[drop_y][x][r_idx] = true;

            // Detect spin for this placement
            SpinType spin = TSpinDetector::detect(
                board, piece, x, drop_y, r, curr.last_move, curr.kick_index
            );

            results.push_back({
                x, drop_y, r, is_hold_action, spin, curr.last_move, curr.kick_index
            });
        }

        // Try Movements
        // 1. Left
        if (x > 0 && board.can_place(piece, r, x - 1, y)) {
            if (!visited[y][x - 1][r_idx]) {
                visited[y][x - 1][r_idx] = true;
                q.push({x - 1, y, r, MoveType::NORMAL, 0});
            }
        }

        // 2. Right
        if (x < Board::WIDTH - 1 && board.can_place(piece, r, x + 1, y)) {
            if (!visited[y][x + 1][r_idx]) {
                visited[y][x + 1][r_idx] = true;
                q.push({x + 1, y, r, MoveType::NORMAL, 0});
            }
        }

        // 3. Soft Drop
        if (y > 0 && board.can_place(piece, r, x, y - 1)) {
            if (!visited[y - 1][x][r_idx]) {
                visited[y - 1][x][r_idx] = true;
                q.push({x, y - 1, r, MoveType::NORMAL, 0});
            }
        }

        // 4. CW Rotation
        Rotation cw_r = rotate_cw(r);
        const auto& cw_kicks = SRSPlus::get_kicks(piece, r, cw_r);
        for (size_t k = 0; k < cw_kicks.size(); ++k) {
            int nx = x + cw_kicks[k].x;
            int ny = y + cw_kicks[k].y;
            if (board.can_place(piece, cw_r, nx, ny)) {
                int cw_idx = static_cast<int>(cw_r);
                if (!visited[ny][nx][cw_idx]) {
                    visited[ny][nx][cw_idx] = true;
                    q.push({nx, ny, cw_r, MoveType::CW, static_cast<int>(k)});
                }
                break; // First valid kick succeeds
            }
        }

        // 5. CCW Rotation
        Rotation ccw_r = rotate_ccw(r);
        const auto& ccw_kicks = SRSPlus::get_kicks(piece, r, ccw_r);
        for (size_t k = 0; k < ccw_kicks.size(); ++k) {
            int nx = x + ccw_kicks[k].x;
            int ny = y + ccw_kicks[k].y;
            if (board.can_place(piece, ccw_r, nx, ny)) {
                int ccw_idx = static_cast<int>(ccw_r);
                if (!visited[ny][nx][ccw_idx]) {
                    visited[ny][nx][ccw_idx] = true;
                    q.push({nx, ny, ccw_r, MoveType::CCW, static_cast<int>(k)});
                }
                break;
            }
        }

        // 6. 180 Rotation
        Rotation r180 = rotate_180(r);
        const auto& r180_kicks = SRSPlus::get_kicks(piece, r, r180);
        for (size_t k = 0; k < r180_kicks.size(); ++k) {
            int nx = x + r180_kicks[k].x;
            int ny = y + r180_kicks[k].y;
            if (board.can_place(piece, r180, nx, ny)) {
                int r180_idx = static_cast<int>(r180);
                if (!visited[ny][nx][r180_idx]) {
                    visited[ny][nx][r180_idx] = true;
                    q.push({nx, ny, r180, MoveType::ROT180, static_cast<int>(k)});
                }
                break;
            }
        }
    }

    return results;
}

} // namespace tetrio
