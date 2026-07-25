#include "engine.hpp"
#include "tspin.hpp"
#include "reachable.hpp"
#include <algorithm>
#include <random>

namespace tetrio {

// SevenBag Implementation
SevenBag::SevenBag(uint64_t seed) {
    reset(seed);
}

void SevenBag::reset(uint64_t seed) {
    if (seed == 0) {
        std::random_device rd;
        seed = (static_cast<uint64_t>(rd()) << 32) | rd();
    }
    rng_.seed(seed);
    bag_ = {PieceType::I, PieceType::J, PieceType::L, PieceType::O, PieceType::S, PieceType::T, PieceType::Z};
    index_ = 0;
    refill();
}

void SevenBag::refill() {
    std::shuffle(bag_.begin(), bag_.end(), rng_);
    index_ = 0;
}

PieceType SevenBag::next() {
    if (index_ >= bag_.size()) {
        refill();
    }
    return bag_[index_++];
}

// TetrioPlayer Implementation
TetrioPlayer::TetrioPlayer(uint64_t seed)
    : last_garbage_hole_(-1) {
    reset(seed);
}

void TetrioPlayer::reset(uint64_t seed) {
    if (seed == 0) {
        std::random_device rd;
        seed = (static_cast<uint64_t>(rd()) << 32) | rd();
    }
    board_.reset();
    bag_.reset(seed);
    state_ = PlayerState();
    hold_piece_ = PieceType::NONE;
    can_hold_ = true;
    queue_.clear();
    garbage_mgr_.reset();
    last_garbage_hole_ = -1;

    for (int i = 0; i < 7; ++i) {
        queue_.push_back(bag_.next());
    }
    spawn_next_piece();
}

void TetrioPlayer::spawn_next_piece() {
    if (queue_.empty()) return;
    current_piece_ = queue_.front();
    queue_.pop_front();
    queue_.push_back(bag_.next());
    can_hold_ = true;

    // Check spawn collision (Top-Out)
    int spawn_x = (current_piece_ == PieceType::O) ? 4 : 3;
    if (!board_.can_place(current_piece_, Rotation::SPAWN, spawn_x, 19)) {
        state_.is_dead = true;
    }
}

std::vector<Placement> TetrioPlayer::get_possible_placements() {
    if (state_.is_dead || current_piece_ == PieceType::NONE) {
        return {};
    }

    std::vector<Placement> normal_placements = ReachableSearch::find_reachable_placements(board_, current_piece_, false);

    if (can_hold_) {
        PieceType next_hold = (hold_piece_ == PieceType::NONE) ? (queue_.empty() ? PieceType::I : queue_.front()) : hold_piece_;
        std::vector<Placement> hold_placements = ReachableSearch::find_reachable_placements(board_, next_hold, true);

        for (auto& hp : hold_placements) {
            hp.hold = true;
            normal_placements.push_back(hp);
        }
    }

    return normal_placements;
}

AttackInfo TetrioPlayer::step(const Placement& placement) {
    AttackInfo info;
    if (state_.is_dead) return info;

    if (placement.hold) {
        if (!hold()) {
            state_.is_dead = true;
            return info;
        }
    }

    PieceType piece_to_place = current_piece_;

    if (!board_.can_place(piece_to_place, placement.rotation, placement.x, placement.y)) {
        state_.is_dead = true;
        return info;
    }

    board_.place_piece(piece_to_place, placement.rotation, placement.x, placement.y);
    state_.total_pieces_placed++;

    SpinType spin = TSpinDetector::detect(
        board_, piece_to_place, placement.x, placement.y,
        placement.rotation, placement.move_type, placement.kick_index
    );

    int lines = board_.clear_lines();
    state_.total_lines_cleared += lines;

    bool all_clear = (lines > 0) && (board_.get_max_height() == 0);

    // Calculate attack and update player state
    info = GarbageManager::calculate_attack(lines, piece_to_place, spin, all_clear, state_.b2b_level, state_.combo);

    state_.b2b_level = info.b2b_level_after;
    state_.combo = info.combo_after;
    state_.total_attack_sent += info.total_attack;

    std::vector<GarbageEntry> tanked_entries = garbage_mgr_.tick_garbage();
    for (const auto& entry : tanked_entries) {
        board_.add_garbage(entry.amount, entry.hole_column);
    }

    if (board_.is_top_out()) {
        state_.is_dead = true;
    } else {
        spawn_next_piece();
    }

    return info;
}

bool TetrioPlayer::hold() {
    if (!can_hold_) return false;

    if (hold_piece_ == PieceType::NONE) {
        hold_piece_ = current_piece_;
        spawn_next_piece();
    } else {
        std::swap(hold_piece_, current_piece_);
    }

    can_hold_ = false;
    return true;
}

std::vector<PieceType> TetrioPlayer::get_queue(size_t n) const {
    std::vector<PieceType> q;
    for (size_t i = 0; i < n && i < queue_.size(); ++i) {
        q.push_back(queue_[i]);
    }
    return q;
}

void TetrioPlayer::add_incoming_garbage(int amount, int delay_ticks, int hole_col) {
    garbage_mgr_.queue_garbage(amount, delay_ticks, hole_col);
}

// TetrioVsGame Implementation
TetrioVsGame::TetrioVsGame(uint64_t seed1, uint64_t seed2)
    : p1_(seed1), p2_(seed2) {}

void TetrioVsGame::reset(uint64_t seed1, uint64_t seed2) {
    p1_.reset(seed1);
    p2_.reset(seed2);
}

AttackInfo TetrioVsGame::step_p1(const Placement& action) {
    AttackInfo info;
    if (is_game_over()) return info;
    info = p1_.step(action);

    int net_attack = p1_.get_garbage_manager().offset_garbage(info.total_attack);
    if (net_attack > 0) {
        p2_.add_incoming_garbage(net_attack, 1, -1);
    }
    return info;
}

AttackInfo TetrioVsGame::step_p2(const Placement& action) {
    AttackInfo info;
    if (is_game_over()) return info;
    info = p2_.step(action);

    int net_attack = p2_.get_garbage_manager().offset_garbage(info.total_attack);
    if (net_attack > 0) {
        p1_.add_incoming_garbage(net_attack, 1, -1);
    }
    return info;
}

int TetrioVsGame::get_winner() const {
    if (!is_game_over()) return 0;
    if (p1_.get_state().is_dead && p2_.get_state().is_dead) return -1;
    return p1_.get_state().is_dead ? 2 : 1;
}

} // namespace tetrio
