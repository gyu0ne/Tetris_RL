#include "garbage.hpp"
#include "board.hpp"
#include <cmath>
#include <algorithm>

namespace tetrio {

// TETR.io Combo Attack Bonus Table by Combo Count
// Combo 0 (1st clear): 0
// Combo 1 (2nd clear): 1
// Combo 2 (3rd clear): 1
// Combo 3 (4th clear): 2
// Combo 4 (5th clear): 2
// Combo 5 (6th clear): 3
// Combo 6 (7th clear): 3
// Combo 7 (8th clear): 4
// Combo 8 (9th clear): 4
// Combo 9 (10th clear): 4
// Combo 10+ (11th+ clear): 5
static const int COMBO_TABLE[] = {0, 1, 1, 2, 2, 3, 3, 4, 4, 4, 5};

static int get_b2b_bonus(int b2b_level) {
    if (b2b_level <= 0) return 0;
    if (b2b_level <= 3) return 1;
    if (b2b_level <= 7) return 2;
    if (b2b_level <= 15) return 3;
    if (b2b_level <= 31) return 4;
    return static_cast<int>(std::floor(1.0 + std::log(1.0 + b2b_level * 0.8)));
}

static int get_combo_bonus(int combo) {
    if (combo <= 0) return 0;
    if (combo < 10) return COMBO_TABLE[combo];
    return 5 + (combo - 10) / 2;
}

AttackInfo GarbageManager::calculate_attack(
    int lines_cleared,
    PieceType piece,
    SpinType spin,
    bool is_all_clear,
    int current_b2b_level,
    int current_combo
) {
    AttackInfo info;
    info.lines_cleared = lines_cleared;
    info.piece_type = piece;
    info.spin = spin;
    info.all_clear = is_all_clear;

    if (lines_cleared == 0) {
        info.b2b_level_after = current_b2b_level;
        info.combo_after = -1; // Combo reset on non-clear placement
        info.total_attack = 0;
        return info;
    }

    // Determine B2B eligibility and base attack
    bool is_b2b = false;
    int base_attack = 0;

    if (spin != SpinType::NONE) {
        is_b2b = true;
        if (spin == SpinType::MINI) {
            if (lines_cleared == 1) base_attack = 0;
            else if (lines_cleared == 2) base_attack = 1;
        } else { // FULL T-Spin
            if (lines_cleared == 1) base_attack = 2;
            else if (lines_cleared == 2) base_attack = 4;
            else if (lines_cleared == 3) base_attack = 6;
        }
    } else if (lines_cleared == 4) { // Quad (Tetris)
        is_b2b = true;
        base_attack = 4;
    } else { // Normal Single, Double, Triple
        if (lines_cleared == 1) base_attack = 0;
        else if (lines_cleared == 2) base_attack = 1;
        else if (lines_cleared == 3) base_attack = 2;
    }

    info.b2b_eligible = is_b2b;
    info.base_attack = base_attack;

    // B2B Leveling
    if (is_b2b) {
        info.b2b_level_after = current_b2b_level + 1;
        info.b2b_bonus = get_b2b_bonus(current_b2b_level);
    } else {
        info.b2b_level_after = 0;
        info.b2b_bonus = 0;
    }

    // Combo calculation
    info.combo_after = (current_combo < 0) ? 0 : current_combo + 1;
    info.combo_bonus = get_combo_bonus(info.combo_after);

    // Total attack calculation
    int raw_attack = base_attack + info.b2b_bonus + info.combo_bonus;
    if (is_all_clear) {
        raw_attack += 10;
    }

    info.total_attack = raw_attack;
    return info;
}

GarbageManager::GarbageManager() {
    reset();
}

void GarbageManager::reset() {
    queue_.clear();
}

void GarbageManager::queue_garbage(int amount, int delay_ticks, int hole_col) {
    if (amount <= 0) return;
    if (hole_col < 0 || hole_col >= Board::WIDTH) {
        static std::mt19937 rand_hole(1337);
        std::uniform_int_distribution<int> dist(0, Board::WIDTH - 1);
        hole_col = dist(rand_hole);
    }
    queue_.push_back({amount, delay_ticks, hole_col});
}

int GarbageManager::offset_garbage(int outgoing_attack) {
    int remaining = outgoing_attack;

    while (remaining > 0 && !queue_.empty()) {
        auto& front = queue_.front();
        if (front.amount <= remaining) {
            remaining -= front.amount;
            queue_.pop_front();
        } else {
            front.amount -= remaining;
            remaining = 0;
        }
    }

    return remaining;
}

std::vector<GarbageEntry> GarbageManager::tick_garbage() {
    std::vector<GarbageEntry> ready;

    for (auto it = queue_.begin(); it != queue_.end();) {
        it->delay_ticks--;
        if (it->delay_ticks <= 0) {
            ready.push_back(*it);
            it = queue_.erase(it);
        } else {
            ++it;
        }
    }

    return ready;
}

int GarbageManager::get_total_queued() const {
    int total = 0;
    for (const auto& entry : queue_) {
        total += entry.amount;
    }
    return total;
}

} // namespace tetrio
