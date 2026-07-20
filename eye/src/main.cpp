#include "camera_manager.hpp"
#include "hand_detector.hpp"
#include "gesture_fsm.hpp"
#include "ipc_sender.hpp"

#include <opencv2/imgproc.hpp>
#include <opencv2/highgui.hpp>

#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <thread>
#include <chrono>
#include <atomic>
#include <string>

static std::atomic<bool> running{true};

void signal_handler(int) { running = false; }

static std::string get_env(const char* name, const char* fallback) {
    const char* val = std::getenv(name);
    return val ? std::string(val) : std::string(fallback);
}

int main(int argc, char* argv[]) {
    std::signal(SIGINT, signal_handler);
    std::signal(SIGTERM, signal_handler);

    std::fprintf(stderr, "[Eye] PalmSync Eye v0.3.0 (model-free) starting\n");

    // ── Configuration ─────────────────────────────────────
    int camera_id = 0;
    int fps = 30;
    std::string ipc_addr = get_env("PALMSYNC_IPC_ADDR", "127.0.0.1");
    int ipc_port = std::atoi(get_env("PALMSYNC_IPC_PORT", "12345").c_str());
    float threshold = 0.40f;  // openness below this = fist (GRAB)
    bool show_window = true;

    for (int i = 1; i < argc; i++) {
        std::string arg = argv[i];
        if (arg == "--camera" && i + 1 < argc) {
            camera_id = std::atoi(argv[++i]);
        } else if (arg == "--fps" && i + 1 < argc) {
            fps = std::atoi(argv[++i]);
        } else if (arg == "--port" && i + 1 < argc) {
            ipc_port = std::atoi(argv[++i]);
        } else if (arg == "--threshold" && i + 1 < argc) {
            threshold = static_cast<float>(std::atof(argv[++i]));
        } else if (arg == "--no-window") {
            show_window = false;
        } else if (arg == "--help") {
            std::printf("PalmSync Eye - Hand Gesture Detection (model-free)\n\n");
            std::printf("Make a FIST to GRAB, OPEN your hand to DROP.\n\n");
            std::printf("Options:\n");
            std::printf("  --camera <id>      Camera device ID (default: 0)\n");
            std::printf("  --fps <n>          Target FPS (default: 30)\n");
            std::printf("  --port <n>         IPC UDP port (default: 12345)\n");
            std::printf("  --threshold <0-1>  Fist sensitivity (default: 0.40)\n");
            std::printf("  --no-window        Run headless (no preview window)\n");
            std::printf("  --help             Show this help\n");
            return 0;
        }
    }

    // ── Camera ────────────────────────────────────────────
    CameraManager camera(camera_id, fps);
    if (!camera.initialize()) {
        std::fprintf(stderr, "[Eye] Camera init failed — is a webcam connected?\n");
        return 1;
    }

    // ── IPC Sender ────────────────────────────────────────
    IpcSender ipc(ipc_addr, ipc_port);
    if (!ipc.initialize()) {
        std::fprintf(stderr, "[Eye] IPC init failed — exiting\n");
        return 1;
    }

    HandDetector detector;
    GestureFSM fsm(threshold, 250);

    // Track the last gesture so the preview can flash it
    std::string last_gesture;
    auto last_gesture_time = std::chrono::steady_clock::now();

    fsm.set_callback([&](GestureEvent event) {
        if (event == GestureEvent::Grab) {
            std::fprintf(stderr, "[Eye] >>> GRAB (fist)\n");
            ipc.send_grab(0.95);
            last_gesture = "GRAB";
        } else if (event == GestureEvent::Drop) {
            std::fprintf(stderr, "[Eye] >>> DROP (open hand)\n");
            ipc.send_drop(0.95);
            last_gesture = "DROP";
        }
        last_gesture_time = std::chrono::steady_clock::now();
    });

    const char* window = "PalmSync Eye";
    if (show_window) cv::namedWindow(window, cv::WINDOW_AUTOSIZE);

    std::fprintf(stderr, "[Eye] Running. FIST = GRAB, OPEN HAND = DROP. "
                         "Press ESC or close the window to quit.\n");

    cv::Mat frame;
    float smoothed = 1.0f;  // exponential moving average of openness
    bool have_hand = false;

    while (running) {
        if (!camera.capture_frame(frame)) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
            continue;
        }

        cv::flip(frame, frame, 1);  // mirror for natural interaction

        float openness = 0.0f, confidence = 0.0f;
        cv::Mat* overlay = show_window ? &frame : nullptr;
        bool found = detector.detect(frame, openness, confidence, overlay);

        if (found) {
            // Smooth to suppress single-frame jitter
            smoothed = have_hand ? (0.6f * smoothed + 0.4f * openness) : openness;
            have_hand = true;
            fsm.update_closure(smoothed);
        } else {
            have_hand = false;
        }

        if (show_window) {
            const char* hand_state = !found ? "no hand"
                                    : (smoothed < threshold ? "FIST" : "OPEN");
            cv::putText(frame, cv::format("%s  openness=%.2f (thr %.2f)",
                        hand_state, smoothed, threshold),
                        {12, 28}, cv::FONT_HERSHEY_SIMPLEX, 0.7,
                        {255, 255, 255}, 2);
            cv::putText(frame, "FIST = GRAB    OPEN HAND = DROP",
                        {12, frame.rows - 16}, cv::FONT_HERSHEY_SIMPLEX, 0.6,
                        {200, 200, 200}, 2);

            // Flash the last gesture for 1 second
            auto since = std::chrono::duration_cast<std::chrono::milliseconds>(
                std::chrono::steady_clock::now() - last_gesture_time).count();
            if (!last_gesture.empty() && since < 1000) {
                cv::putText(frame, last_gesture, {frame.cols / 2 - 90, frame.rows / 2},
                            cv::FONT_HERSHEY_SIMPLEX, 2.0,
                            last_gesture == "GRAB" ? cv::Scalar(0, 220, 0)
                                                   : cv::Scalar(0, 160, 255), 4);
            }

            cv::imshow(window, frame);
            int key = cv::waitKey(1);
            if (key == 27) running = false;  // ESC
            if (cv::getWindowProperty(window, cv::WND_PROP_VISIBLE) < 1)
                running = false;  // window closed
        } else {
            std::this_thread::sleep_for(std::chrono::milliseconds(1000 / fps));
        }
    }

    std::fprintf(stderr, "[Eye] Shutting down\n");
    if (show_window) cv::destroyAllWindows();
    camera.release();
    ipc.shutdown();
    return 0;
}
