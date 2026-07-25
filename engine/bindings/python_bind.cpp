#include <pybind11/pybind11.h>
#include <pybind11/stl.h>
#include <pybind11/functional.h>
#include "tetrio_types.hpp"
#include "board.hpp"
#include "tetrio_srs_plus.hpp"
#include "tspin.hpp"
#include "garbage.hpp"
#include "engine.hpp"
#include "beam_search.hpp"

namespace py = pybind11;
using namespace tetrio;

PYBIND11_MODULE(tetrio_engine, m) {
    m.doc() = "TETR.io Exact Tetris Engine and 1v1 Battle Simulator";

    // Enums
    py::enum_<PieceType>(m, "PieceType")
        .value("NONE", PieceType::NONE)
        .value("I", PieceType::I)
        .value("J", PieceType::J)
        .value("L", PieceType::L)
        .value("O", PieceType::O)
        .value("S", PieceType::S)
        .value("T", PieceType::T)
        .value("Z", PieceType::Z)
        .value("GARBAGE", PieceType::GARBAGE)
        .export_values();

    py::enum_<Rotation>(m, "Rotation")
        .value("SPAWN", Rotation::SPAWN)
        .value("RIGHT", Rotation::RIGHT)
        .value("REVERSE", Rotation::REVERSE)
        .value("LEFT", Rotation::LEFT)
        .export_values();

    py::enum_<SpinType>(m, "SpinType")
        .value("NONE", SpinType::NONE)
        .value("MINI", SpinType::MINI)
        .value("FULL", SpinType::FULL)
        .export_values();

    py::enum_<MoveType>(m, "MoveType")
        .value("NORMAL", MoveType::NORMAL)
        .value("CW", MoveType::CW)
        .value("CCW", MoveType::CCW)
        .value("ROT180", MoveType::ROT180)
        .export_values();

    // Structs
    py::class_<Point>(m, "Point")
        .def_readwrite("x", &Point::x)
        .def_readwrite("y", &Point::y);

    py::class_<RotateResult>(m, "RotateResult")
        .def_readwrite("success", &RotateResult::success)
        .def_readwrite("new_x", &RotateResult::new_x)
        .def_readwrite("new_y", &RotateResult::new_y)
        .def_readwrite("new_rot", &RotateResult::new_rot)
        .def_readwrite("kick_index", &RotateResult::kick_index);

    py::class_<Placement>(m, "Placement")
        .def(py::init<int, int, Rotation, bool, SpinType, MoveType, int>(),
             py::arg("x") = 0, py::arg("y") = 0,
             py::arg("rotation") = Rotation::SPAWN, py::arg("hold") = false,
             py::arg("spin") = SpinType::NONE, py::arg("move_type") = MoveType::NORMAL,
             py::arg("kick_index") = 0)
        .def_readwrite("x", &Placement::x)
        .def_readwrite("y", &Placement::y)
        .def_readwrite("rotation", &Placement::rotation)
        .def_readwrite("hold", &Placement::hold)
        .def_readwrite("spin", &Placement::spin)
        .def_readwrite("move_type", &Placement::move_type)
        .def_readwrite("kick_index", &Placement::kick_index);

    py::class_<AttackInfo>(m, "AttackInfo")
        .def_readwrite("lines_cleared", &AttackInfo::lines_cleared)
        .def_readwrite("piece_type", &AttackInfo::piece_type)
        .def_readwrite("spin", &AttackInfo::spin)
        .def_readwrite("b2b_eligible", &AttackInfo::b2b_eligible)
        .def_readwrite("all_clear", &AttackInfo::all_clear)
        .def_readwrite("base_attack", &AttackInfo::base_attack)
        .def_readwrite("b2b_bonus", &AttackInfo::b2b_bonus)
        .def_readwrite("combo_bonus", &AttackInfo::combo_bonus)
        .def_readwrite("total_attack", &AttackInfo::total_attack)
        .def_readwrite("b2b_level_after", &AttackInfo::b2b_level_after)
        .def_readwrite("combo_after", &AttackInfo::combo_after);

    py::class_<GarbageEntry>(m, "GarbageEntry")
        .def_readwrite("amount", &GarbageEntry::amount)
        .def_readwrite("delay_ticks", &GarbageEntry::delay_ticks)
        .def_readwrite("hole_column", &GarbageEntry::hole_column);

    py::class_<PlayerState>(m, "PlayerState")
        .def_readwrite("b2b_level", &PlayerState::b2b_level)
        .def_readwrite("combo", &PlayerState::combo)
        .def_readwrite("total_attack_sent", &PlayerState::total_attack_sent)
        .def_readwrite("total_lines_cleared", &PlayerState::total_lines_cleared)
        .def_readwrite("total_pieces_placed", &PlayerState::total_pieces_placed)
        .def_readwrite("is_dead", &PlayerState::is_dead);

    // SRSPlus
    py::class_<SRSPlus>(m, "SRSPlus")
        .def_static("try_rotate", &SRSPlus::try_rotate);

    // Board
    py::class_<Board>(m, "Board")
        .def(py::init<>())
        .def("reset", &Board::reset)
        .def("is_occupied", &Board::is_occupied)
        .def("get_cell", &Board::get_cell)
        .def("can_place", &Board::can_place)
        .def("place_piece", &Board::place_piece)
        .def("clear_lines", &Board::clear_lines)
        .def("add_garbage", &Board::add_garbage)
        .def("is_top_out", &Board::is_top_out)
        .def("get_column_height", &Board::get_column_height)
        .def("get_max_height", &Board::get_max_height)
        .def("count_holes", &Board::count_holes)
        .def("get_bumpiness", &Board::get_bumpiness)
        .def("get_rows", [](const Board& b) {
            auto rows = b.get_rows();
            std::vector<uint16_t> v(rows.begin(), rows.end());
            return v;
        });

    // TetrioPlayer
    py::class_<TetrioPlayer>(m, "TetrioPlayer")
        .def(py::init<uint64_t>(), py::arg("seed") = 1337)
        .def("reset", &TetrioPlayer::reset, py::arg("seed") = 1337)
        .def("get_possible_placements", &TetrioPlayer::get_possible_placements)
        .def("step", &TetrioPlayer::step)
        .def("hold", &TetrioPlayer::hold)
        .def("get_board", &TetrioPlayer::get_board, py::return_value_policy::reference)
        .def("get_current_piece", &TetrioPlayer::get_current_piece)
        .def("get_hold_piece", &TetrioPlayer::get_hold_piece)
        .def("can_hold", &TetrioPlayer::can_hold)
        .def("get_queue", &TetrioPlayer::get_queue, py::arg("n") = 5)
        .def("get_state", &TetrioPlayer::get_state, py::return_value_policy::reference)
        .def("add_incoming_garbage", &TetrioPlayer::add_incoming_garbage,
             py::arg("amount"), py::arg("delay_ticks") = 1, py::arg("hole_col") = -1);

    // TetrioVsGame
    py::class_<TetrioVsGame>(m, "TetrioVsGame")
        .def(py::init<uint64_t, uint64_t>(), py::arg("seed1") = 1337, py::arg("seed2") = 4242)
        .def("reset", &TetrioVsGame::reset, py::arg("seed1") = 1337, py::arg("seed2") = 4242)
        .def("step_p1", &TetrioVsGame::step_p1)
        .def("step_p2", &TetrioVsGame::step_p2)
        .def("get_p1", py::overload_cast<>(&TetrioVsGame::get_p1), py::return_value_policy::reference)
        .def("get_p2", py::overload_cast<>(&TetrioVsGame::get_p2), py::return_value_policy::reference)
        .def("is_game_over", &TetrioVsGame::is_game_over)
        .def("get_winner", &TetrioVsGame::get_winner);

    // BeamSearchEngine
    py::class_<BeamSearchEngine>(m, "BeamSearchEngine")
        .def_static("find_best_move", &BeamSearchEngine::find_best_move,
                    py::arg("player"), py::arg("depth") = 4, py::arg("beam_width") = 16,
                    py::arg("eval_fn") = nullptr)
        .def_static("default_heuristic", &BeamSearchEngine::default_heuristic);
}
