#include "tetrio_srs_plus.hpp"
#include <iostream>
#include <cassert>

using namespace tetrio;

void test_srs_plus_kicks() {
    std::cout << "[Test] SRS+ Kick Tables..." << std::endl;

    // Test J piece 0 -> R
    const auto& kicks = SRSPlus::get_kicks(PieceType::J, Rotation::SPAWN, Rotation::RIGHT);
    assert(kicks.size() == 5);
    assert(kicks[0].x == 0 && kicks[0].y == 0);
    assert(kicks[1].x == -1 && kicks[1].y == 0);

    // Test 180 rotation for T piece 0 -> 2
    const auto& kicks180 = SRSPlus::get_kicks(PieceType::T, Rotation::SPAWN, Rotation::REVERSE);
    assert(kicks180.size() == 7);
    assert(kicks180[0].x == 0 && kicks180[0].y == 0);
    assert(kicks180[1].x == 0 && kicks180[1].y == 1);

    // Test I piece 180 rotation
    const auto& i_kicks180 = SRSPlus::get_kicks(PieceType::I, Rotation::SPAWN, Rotation::REVERSE);
    assert(i_kicks180.size() == 6);
    assert(i_kicks180[1].x == -1 && i_kicks180[1].y == 0);

    std::cout << "  -> PASSED!" << std::endl;
}
