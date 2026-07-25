#ifndef TETRIO_ENGINE_HPP
#define TETRIO_ENGINE_HPP

#include "board.hpp"
#include "tetrio_types.hpp"
#include "garbage.hpp"
#include <deque>
#include <random>
#include <array>
#include <memory>

namespace tetrio {

class SevenBag {
public:
    explicit SevenBag(uint64_t seed = 1337);
    PieceType next();
    void reset(uint64_t seed = 1337);

private:
    void refill();
    std::mt19937_64 rng_;
    std::vector<PieceType> bag_;
    size_t index_;
};

class TetrioPlayer {
public:
    explicit TetrioPlayer(uint64_t seed = 1337);
    void reset(uint64_t seed = 1337);

    // Get current available placements
    std::vector<Placement> get_possible_placements();

    // Execute placement move (hard drop / lock)
    AttackInfo step(const Placement& placement);

    // Perform Hold action without hard-dropping
    bool hold();

    // Game state getters
    const Board& get_board() const { return board_; }
    Board& get_board_mutable() { return board_; }
    PieceType get_current_piece() const { return current_piece_; }
    PieceType get_hold_piece() const { return hold_piece_; }
    bool can_hold() const { return can_hold_; }
    std::vector<PieceType> get_queue(size_t n = 5) const;
    const PlayerState& get_state() const { return state_; }
    GarbageManager& get_garbage_manager() { return garbage_mgr_; }
    const GarbageManager& get_garbage_manager() const { return garbage_mgr_; }

    void add_incoming_garbage(int amount, int delay_ticks, int hole_col);

private:
    void spawn_next_piece();

    Board board_;
    SevenBag bag_;
    PieceType current_piece_;
    PieceType hold_piece_;
    bool can_hold_;
    std::deque<PieceType> queue_;
    PlayerState state_;
    GarbageManager garbage_mgr_;
    std::mt19937 rng_;
    int last_garbage_hole_;
};

class TetrioVsGame {
public:
    TetrioVsGame(uint64_t seed1 = 1337, uint64_t seed2 = 4242);
    void reset(uint64_t seed1 = 1337, uint64_t seed2 = 4242);

    AttackInfo step_p1(const Placement& action);
    AttackInfo step_p2(const Placement& action);

    TetrioPlayer& get_p1() { return p1_; }
    TetrioPlayer& get_p2() { return p2_; }
    const TetrioPlayer& get_p1() const { return p1_; }
    const TetrioPlayer& get_p2() const { return p2_; }

    bool is_game_over() const { return p1_.get_state().is_dead || p2_.get_state().is_dead; }
    int get_winner() const;

private:
    TetrioPlayer p1_;
    TetrioPlayer p2_;
};

} // namespace tetrio

#endif // TETRIO_ENGINE_HPP
