#include "engine.hpp"
#include <iostream>
#include <cassert>

using namespace tetrio;

void test_srs_plus_kicks();
void test_tspin_detection();
void test_garbage_formulas();

int main() {
    std::cout << "========================================" << std::endl;
    std::cout << " Running TETR.io Engine Parity Tests   " << std::endl;
    std::cout << "========================================" << std::endl;

    test_srs_plus_kicks();
    test_tspin_detection();
    test_garbage_formulas();

    std::cout << "[Test] Full 1v1 VS Game Simulation..." << std::endl;
    TetrioVsGame game(1337, 4242);

    auto p1_placements = game.get_p1().get_possible_placements();
    assert(!p1_placements.empty());

    // Execute first placement
    AttackInfo info = game.step_p1(p1_placements[0]);
    assert(game.get_p1().get_state().total_pieces_placed == 1);

    std::cout << "  -> PASSED!" << std::endl;

    std::cout << "========================================" << std::endl;
    std::cout << " ALL TETR.IO ENGINE TESTS PASSED 100%! " << std::endl;
    std::cout << "========================================" << std::endl;

    return 0;
}
