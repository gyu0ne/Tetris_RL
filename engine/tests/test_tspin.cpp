#include "tspin.hpp"
#include "board.hpp"
#include <iostream>
#include <cassert>

using namespace tetrio;

void test_tspin_detection() {
    std::cout << "[Test] T-Spin 3-Corner Rules..." << std::endl;

    Board board;

    // Create a T-Spin notch at (x=2, y=1)
    // Board blocks surrounding (2,1)
    board.place_piece(PieceType::O, Rotation::SPAWN, 0, 0); // covers (0,0), (1,0), (0,1), (1,1)
    board.place_piece(PieceType::O, Rotation::SPAWN, 3, 0); // covers (3,0), (4,0), (3,1), (4,1)

    // Last move: CW rotation at (2, 1) pointing UP
    SpinType spin = TSpinDetector::detect(
        board, PieceType::T, 2, 1, Rotation::SPAWN, MoveType::CW, 0
    );

    // Front corners: (1,2) empty, (3,2) empty
    // Back corners: (1,0) filled, (3,0) filled, floor (y=-1) occupied
    // Total corners occupied: (1,0) and (3,0) -> 2 corners. Need 3 for T-Spin.
    // Let's add block at (1,2) to make 3 corners
    board.place_piece(PieceType::I, Rotation::RIGHT, 0, 1); // covers (1,2), (1,1), (1,0), (1,-1)

    SpinType spin2 = TSpinDetector::detect(
        board, PieceType::T, 2, 1, Rotation::SPAWN, MoveType::CW, 0
    );

    assert(spin2 != SpinType::NONE);

    std::cout << "  -> PASSED!" << std::endl;
}
