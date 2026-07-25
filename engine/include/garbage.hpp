#ifndef TETRIO_GARBAGE_HPP
#define TETRIO_GARBAGE_HPP

#include "tetrio_types.hpp"
#include <deque>
#include <random>

namespace tetrio {

class GarbageManager {
public:
    static AttackInfo calculate_attack(
        int lines_cleared,
        PieceType piece,
        SpinType spin,
        bool is_all_clear,
        int current_b2b_level,
        int current_combo
    );

    GarbageManager();
    void reset();

    // Add incoming garbage attack to tanking queue
    void queue_garbage(int amount, int delay_ticks, int hole_col);

    // Cancel incoming garbage with outgoing attack. Returns remaining outgoing attack sent to opponent.
    int offset_garbage(int outgoing_attack);

    // Process tanking delay. Returns garbage entries ready to rise on board.
    std::vector<GarbageEntry> tick_garbage();

    const std::deque<GarbageEntry>& get_queue() const { return queue_; }
    int get_total_queued() const;

private:
    std::deque<GarbageEntry> queue_;
};

} // namespace tetrio

#endif // TETRIO_GARBAGE_HPP
