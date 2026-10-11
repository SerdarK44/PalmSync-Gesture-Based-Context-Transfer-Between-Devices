cmake_minimum_required(VERSION 3.20)

# Product version from the repo-root VERSION file (bumped by release.ps1), so
# `palmsync-eye --version` matches the installer and the other components.
set(PALMSYNC_VERSION "0.0.0")
if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../VERSION")
    file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/../VERSION" PALMSYNC_VERSION LIMIT_COUNT 1)
    string(STRIP "${PALMSYNC_VERSION}" PALMSYNC_VERSION)
endif()
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/../VERSION")

project(palmsync-eye VERSION ${PALMSYNC_VERSION} LANGUAGES CXX)
add_compile_definitions(PALMSYNC_VERSION="${PALMSYNC_VERSION}")

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
option(BUILD_TESTS "Build unit tests" ON)

if(BUILD_TESTS)
    enable_testing()
endif()

# ── Main app (needs OpenCV) ──────────────────────────────
if(PALMSYNC_BUILD_APP)
    find_package(OpenCV REQUIRED COMPONENTS core dnn imgproc videoio highgui objdetect)

    set(EYE_SOURCES
        src/main.cpp
        src/camera_manager.cpp
        src/hand_backend.cpp
        src/hand_detector.cpp
        src/landmark_hand_detector.cpp
        src/gesture_fsm.cpp
        src/test_guide.cpp
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

    # fetch-models.ps1 verifies these files before the supported build scripts
    # configure CMake. Copy them beside the executable for local camera tests.
    set(PALMSYNC_MODEL_DIR ${CMAKE_CURRENT_SOURCE_DIR}/models)
    set(PALMSYNC_MODEL_ASSETS
        palm_detection_mediapipe_2023feb.onnx
        handpose_estimation_mediapipe_2023feb.onnx
        LICENSE.opencv-mediapipe.txt
        models.json)
    foreach(MODEL_ASSET IN LISTS PALMSYNC_MODEL_ASSETS)
        if(EXISTS "${PALMSYNC_MODEL_DIR}/${MODEL_ASSET}")
            add_custom_command(TARGET palmsync-eye POST_BUILD
                COMMAND ${CMAKE_COMMAND} -E make_directory
                    $<TARGET_FILE_DIR:palmsync-eye>/models
                COMMAND ${CMAKE_COMMAND} -E copy_if_different
                    "${PALMSYNC_MODEL_DIR}/${MODEL_ASSET}"
                    $<TARGET_FILE_DIR:palmsync-eye>/models/${MODEL_ASSET})
        else()
            message(WARNING "Camera 2.0 asset missing: ${MODEL_ASSET}; auto mode will use classic detector")
        endif()
    endforeach()

    if(WIN32)
        target_link_libraries(palmsync-eye PRIVATE ws2_32 shell32)
    endif()

    if(BUILD_TESTS)
        # Detector test needs OpenCV, so it lives with the app build.
        add_executable(palmsync-detector-test
            tests/detector_test.cpp
            src/hand_detector.cpp
        )
        target_include_directories(palmsync-detector-test PRIVATE include)
        target_link_libraries(palmsync-detector-test PRIVATE ${OpenCV_LIBS})
        add_test(NAME DetectorTests COMMAND palmsync-detector-test)

        add_executable(palmsync-landmark-test
            tests/landmark_test.cpp
            src/landmark_hand_detector.cpp
        )
        target_include_directories(palmsync-landmark-test PRIVATE include)
        target_compile_definitions(palmsync-landmark-test PRIVATE
            PALMSYNC_TEST_MODEL_DIR="${CMAKE_CURRENT_SOURCE_DIR}/models")
        target_link_libraries(palmsync-landmark-test PRIVATE ${OpenCV_LIBS})
        add_test(NAME LandmarkTests COMMAND palmsync-landmark-test)
    endif()
endif()

# ── Tests ────────────────────────────────────────────────
if(BUILD_TESTS)
    add_executable(palmsync-eye-tests
        tests/gesture_test.cpp
        src/gesture_fsm.cpp
    )
    target_include_directories(palmsync-eye-tests PRIVATE include)

    if(WIN32)
        target_link_libraries(palmsync-eye-tests PRIVATE ws2_32)
    endif()

    add_test(NAME GestureTests COMMAND palmsync-eye-tests)

    add_executable(palmsync-backend-tests
        tests/backend_test.cpp
        src/hand_backend.cpp
    )
    target_include_directories(palmsync-backend-tests PRIVATE include)
    add_test(NAME BackendTests COMMAND palmsync-backend-tests)

    add_executable(palmsync-guide-tests
        tests/guide_test.cpp
        src/test_guide.cpp
        src/gesture_fsm.cpp
    )
    target_include_directories(palmsync-guide-tests PRIVATE include)
    add_test(NAME GuideTests COMMAND palmsync-guide-tests)

    # Replays real --metrics recordings (tests/fixtures) through GestureFSM.
    add_library(palmsync-replay STATIC src/metrics_replay.cpp src/gesture_fsm.cpp)
    target_include_directories(palmsync-replay PUBLIC include)

    add_executable(palmsync-replay-tests tests/replay_test.cpp)
    target_link_libraries(palmsync-replay-tests PRIVATE palmsync-replay)
    target_compile_definitions(palmsync-replay-tests PRIVATE
        PALMSYNC_FIXTURE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/tests/fixtures")
    add_test(NAME ReplayTests COMMAND palmsync-replay-tests)

    add_executable(palmsync-eye-replay tests/replay_tool.cpp)
    target_link_libraries(palmsync-eye-replay PRIVATE palmsync-replay)
endif()
