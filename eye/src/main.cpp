#include "camera_manager.hpp"
#include "gesture_fsm.hpp"
#include "ipc_sender.hpp"

#include <opencv2/imgproc.hpp>
#include <csignal>
#include <cstdio>
#include <thread>
#include <chrono>
#include <atomic>
#include <vector>

static std::atomic<bool> running{true};

void signal_handler(int) {
    running = false;
}

// Placeholder: in production this calls MediaPipe.
// For now returns empty landmarks so the loop compiles.
static bool detect_hand(const cv::Mat& /*frame*/,
                        std::vector<HandLandmark>& landmarks) {
    // TODO: integrate MediaPipe Hands C++ API here
    landmarks.clear();
    return false;
}

int main() {
    std::signal(SIGINT, signal_handler);
    std::signal(SIGTERM, signal_handler);

    std::fprintf(stderr, "[Eye] PalmSync Eye v0.1.0 starting\n");

    CameraManager camera(0, 30);
    if (!camera.initialize()) {
        std::fprintf(stderr, "[Eye] Camera init failed — exiting\n");
        return 1;
    }

    IpcSender ipc("127.0.0.1", 12345);
    if (!ipc.initialize()) {
        std::fprintf(stderr, "[Eye] IPC init failed — exiting\n");
        return 1;
    }

    GestureFSM fsm(0.3f, 200);
    fsm.set_callback([&ipc](GestureEvent event) {
        switch (event) {
            case GestureEvent::Grab:
                std::fprintf(stderr, "[Eye] >>> GRAB detected\n");
                ipc.send_grab(0.95);
                break;
            case GestureEvent::Drop:
                std::fprintf(stderr, "[Eye] >>> DROP detected\n");
                ipc.send_drop(0.95);
                break;
            default:
                break;
        }
    });

    cv::Mat frame;
    std::vector<HandLandmark> landmarks;

    std::fprintf(stderr, "[Eye] Main loop running at 30 FPS\n");

    while (running) {
        if (!camera.capture_frame(frame)) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
            continue;
        }

        if (detect_hand(frame, landmarks)) {
            fsm.update(landmarks);
        }

        std::this_thread::sleep_for(std::chrono::milliseconds(33));
    }

    std::fprintf(stderr, "[Eye] Shutting down\n");
    camera.release();
    ipc.shutdown();
    return 0;
}
