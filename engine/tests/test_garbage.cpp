#include "garbage.hpp"
#include <iostream>
#include <cassert>

using namespace tetrio;

void test_garbage_formulas() {
    std::cout << "[Test] Garbage & B2B Formulas..." << std::endl;

    // 1. Tetris (Quad) Attack = 4, B2B level increases to 1, combo starts at 0
    AttackInfo atk1 = GarbageManager::calculate_attack(4, PieceType::I, SpinType::NONE, false, 0, -1);
    assert(atk1.total_attack == 4);
    assert(atk1.b2b_level_after == 1);
    assert(atk1.combo_after == 0);

    // 2. Consecutive TSD (T-Spin Double) at B2B Level 1, Combo 0 -> Combo 1
    // Base attack = 4, B2B Level 1 bonus = +1, Combo 1 bonus = +1 -> Total = 6
    AttackInfo atk2 = GarbageManager::calculate_attack(2, PieceType::T, SpinType::FULL, false, 1, 0);
    assert(atk2.base_attack == 4);
    assert(atk2.b2b_bonus == 1);
    assert(atk2.combo_bonus == 1);
    assert(atk2.total_attack == 6);
    assert(atk2.b2b_level_after == 2);
    assert(atk2.combo_after == 1);

    // 3. Normal Single breaks B2B
    AttackInfo atk3 = GarbageManager::calculate_attack(1, PieceType::J, SpinType::NONE, false, 2, 1);
    assert(atk3.b2b_level_after == 0);

    // 4. Garbage Offsetting
    GarbageManager mgr;
    mgr.queue_garbage(4, 1, 0);
    int net_sent = mgr.offset_garbage(6);
    assert(net_sent == 2); // 6 - 4 = 2 net attack sent
    assert(mgr.get_total_queued() == 0);

    std::cout << "  -> PASSED!" << std::endl;
}
