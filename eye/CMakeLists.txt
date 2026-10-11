cmake_minimum_required(VERSION 3.20)
project(palmsync-eye VERSION 0.2.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Stop windows.h (pulled in via winsock2) from defining min/max macros, which
# otherwise clobber std::min/std::max and OpenCV headers.
if(WIN32)
    add_compile_definitions(NOMINMAX WIN32_LEAN_AND_MEAN)
endif()
# Sources and log messages are UTF-8 (em dashes, arrows); without this MSVC
# reads them in the system code page.
if(MSVC)
    add_compile_options(/utf-8)
endif()

# ── Options ──────────────────────────────────────────────
option(PALMSYNC_USE_GPU "Enable GPU acceleration" OFF)
option(PALMSYNC_BUILD_APP "Build the camera app (requires OpenCV)" ON)

# ── Main app (needs OpenCV) ──────────────────────────────
if(PALMSYNC_BUILD_APP)
    find_package(OpenCV REQUIRED COMPONENTS core imgproc videoio highgui objdetect)

    set(EYE_SOURCES
        src/main.cpp
        src/camera_manager.cpp
        src/hand_detector.cpp
        src/gesture_fsm.cpp
        src/ipc_sender.cpp
        src/log.cpp
    )

    add_executable(palmsync-eye ${EYE_SOURCES})

    target_include_directories(palmsync-eye PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/include
    )

    target_link_libraries(palmsync-eye PRIVATE ${OpenCV_LIBS})

    # Face model used to keep faces out of the hand search (see hand_detector).
    add_custom_command(TARGET palmsync-eye POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            ${CMAKE_CURRENT_SOURCE_DIR}/data/haarcascade_frontalface_default.xml
            $<TARGET_FILE_DIR:palmsync-eye>/haarcascade_frontalface_default.xml
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            ${CMAKE_CURRENT_SOURCE_DIR}/data/haarcascade_profileface.xml
            $<TARGET_FILE_DIR:palmsync-eye>/haarcascade_profileface.xml)

    if(WIN32)
        target_link_libraries(palmsync-eye PRIVATE ws2_32 shell32)
    endif()

    # Detector test needs OpenCV, so it lives with the app build.
    add_executable(palmsync-detector-test
        tests/detector_test.cpp
        src/hand_detector.cpp
    )
    target_include_directories(palmsync-detector-test PRIVATE include)
    target_link_libraries(palmsync-detector-test PRIVATE ${OpenCV_LIBS})
endif()

# ── Tests ────────────────────────────────────────────────
option(BUILD_TESTS "Build unit tests" ON)
if(BUILD_TESTS)
    enable_testing()

    add_executable(palmsync-eye-tests
        tests/gesture_test.cpp
        src/gesture_fsm.cpp
    )
    target_include_directories(palmsync-eye-tests PRIVATE include)

    if(WIN32)
        target_link_libraries(palmsync-eye-tests PRIVATE ws2_32)
    endif()

    add_test(NAME GestureTests COMMAND palmsync-eye-tests)
endif()
