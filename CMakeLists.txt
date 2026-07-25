cmake_minimum_required(VERSION 3.20)
project(tetrio_engine_project LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include_directories(engine/include)

set(ENGINE_SOURCES
    engine/src/tetrio_srs_plus.cpp
    engine/src/board.cpp
    engine/src/tspin.cpp
    engine/src/garbage.cpp
    engine/src/engine.cpp
    engine/src/reachable.cpp
    engine/src/beam_search.cpp
)

add_executable(unit_tests
    engine/tests/test_engine.cpp
    engine/tests/test_srs_plus.cpp
    engine/tests/test_tspin.cpp
    engine/tests/test_garbage.cpp
    ${ENGINE_SOURCES}
)

find_package(Python 3.10 COMPONENTS Interpreter Development REQUIRED)

execute_process(
    COMMAND "${Python_EXECUTABLE}" -c "import pybind11; print(pybind11.get_cmake_dir())"
    OUTPUT_VARIABLE pybind11_DIR
    OUTPUT_STRIP_TRAILING_WHITESPACE
)
list(APPEND CMAKE_PREFIX_PATH "${pybind11_DIR}")
find_package(pybind11 REQUIRED)

pybind11_add_module(tetrio_engine engine/bindings/python_bind.cpp ${ENGINE_SOURCES})
