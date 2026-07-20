cmake_minimum_required(VERSION 3.20)
project(palmsync-eye VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# ── Options ──────────────────────────────────────────────
option(PALMSYNC_STATIC_LINK "Link OpenCV statically" OFF)
option(PALMSYNC_USE_GPU     "Enable GPU acceleration"  OFF)

# ── Dependencies ─────────────────────────────────────────
find_package(OpenCV REQUIRED COMPONENTS core videoio imgproc)
# MediaPipe is vendored / fetched separately — see README

# ── Sources ──────────────────────────────────────────────
set(EYE_SOURCES
    src/main.cpp
    src/camera_manager.cpp
    src/gesture_fsm.cpp
    src/ipc_sender.cpp
)

add_executable(palmsync-eye ${EYE_SOURCES})

target_include_directories(palmsync-eye PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${OpenCV_INCLUDE_DIRS}
)

target_link_libraries(palmsync-eye PRIVATE ${OpenCV_LIBS})

# Hide console on Windows release builds
if(WIN32)
    set_target_properties(palmsync-eye PROPERTIES
        WIN32_EXECUTABLE $<CONFIG:Release>
    )
    target_link_libraries(palmsync-eye PRIVATE ws2_32)
endif()

# ── Tests ────────────────────────────────────────────────
option(BUILD_TESTS "Build unit tests" ON)
if(BUILD_TESTS)
    enable_testing()
    find_package(GTest QUIET)
    if(GTest_FOUND)
        add_executable(palmsync-eye-tests tests/gesture_test.cpp src/gesture_fsm.cpp)
        target_include_directories(palmsync-eye-tests PRIVATE include)
        target_link_libraries(palmsync-eye-tests PRIVATE GTest::gtest GTest::gtest_main)
        add_test(NAME GestureTests COMMAND palmsync-eye-tests)
    endif()
endif()
