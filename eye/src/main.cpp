#include "camera_manager.hpp"
#include "hand_detector.hpp"
#include "gesture_fsm.hpp"
#include "ipc_sender.hpp"
#include "log.hpp"

#include <opencv2/imgproc.hpp>
#include <opencv2/highgui.hpp>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/videoio/registry.hpp>
#include <opencv2/core/utils/logger.hpp>

#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <ctime>
#include <thread>
#include <chrono>
#include <atomic>
#include <string>
#include <algorithm>

#ifdef _WIN32
#include <windows.h>

// ── Preview pipe ───────────────────────────────────────────
// The tray UI hosts a named-pipe server and shows these JPEG frames as the
// live camera preview. If the UI isn't listening (window closed, restarted)
// we keep retrying every couple of seconds instead of giving up for good.
static HANDLE      g_preview_pipe = INVALID_HANDLE_VALUE;
static std::string g_preview_name;
static int         g_preview_seq  = 0;
static std::chrono::steady_clock::time_point g_preview_retry{};

static void preview_pipe_try_open() {
    if (g_preview_name.empty() || g_preview_pipe != INVALID_HANDLE_VALUE) return;
    auto now = std::chrono::steady_clock::now();
    if (now < g_preview_retry) return;
    g_preview_retry = now + std::chrono::seconds(2);

    g_preview_pipe = CreateFileA(g_preview_name.c_str(),
        GENERIC_WRITE, 0, NULL, OPEN_EXISTING, 0, NULL);
    if (g_preview_pipe != INVALID_HANDLE_VALUE)
        EYE_LOG("[Eye] Connected to preview pipe %s\n", g_preview_name.c_str());
}

static bool preview_pipe_active() {
    preview_pipe_try_open();
    return g_preview_pipe != INVALID_HANDLE_VALUE;
}

static void preview_pipe_write(const cv::Mat& frame) {
    if (g_preview_pipe == INVALID_HANDLE_VALUE) return;
    if (++g_preview_seq % 3 != 0) return;  // ~10 fps at 30 fps camera

    cv::Mat small;
    cv::resize(frame, small, cv::Size(320, 240));

    std::vector<uchar> buf;
    cv::imencode(".jpg", small, buf, {cv::IMWRITE_JPEG_QUALITY, 60});

    uint32_t len = static_cast<uint32_t>(buf.size());
    DWORD written;
    if (!WriteFile(g_preview_pipe, &len, 4, &written, NULL) ||
        !WriteFile(g_preview_pipe, buf.data(), len, &written, NULL)) {
        EYE_LOG("[Eye] Preview pipe closed — will retry\n");
        CloseHandle(g_preview_pipe);
        g_preview_pipe = INVALID_HANDLE_VALUE;
    }
}
#else
static bool preview_pipe_active() { return false; }
static void preview_pipe_write(const cv::Mat&) {}
#endif

static std::atomic<bool> running{true};

void signal_handler(int) { running = false; }

static std::string get_env(const char* name, const char* fallback) {
    const char* val = std::getenv(name);
    return val ? std::string(val) : std::string(fallback);
}

#ifdef _WIN32
#include <shellapi.h>
// Value of a "--name value" argument from the UNICODE command line. argv[] is
// in the ANSI code page and mangles paths like C:\Users\Çağrı\Belgeler.
static std::wstring wide_arg(const wchar_t* name) {
    int n = 0;
    LPWSTR* wargv = CommandLineToArgvW(GetCommandLineW(), &n);
    std::wstring value;
    if (wargv) {
        for (int i = 1; i + 1 < n; ++i)
            if (std::wstring(wargv[i]) == name) { value = wargv[i + 1]; break; }
        LocalFree(wargv);
    }
    return value;
}
// Contents of a file next to the exe (Unicode-safe: the install folder may
// contain characters the ANSI code page can't represent).
static std::string read_beside_exe(const wchar_t* name) {
    wchar_t exe[MAX_PATH * 2];
    DWORD n = GetModuleFileNameW(NULL, exe, MAX_PATH * 2);
    if (n == 0 || n >= MAX_PATH * 2) return {};
    std::wstring path(exe, n);
    path = path.substr(0, path.find_last_of(L"\\/") + 1) + name;

    std::string data;
    FILE* f = _wfopen(path.c_str(), L"rb");
    if (!f) return {};
    char buf[64 * 1024];
    size_t got;
    while ((got = std::fread(buf, 1, sizeof(buf), f)) > 0) data.append(buf, got);
    std::fclose(f);
    return data;
}
#else
static std::wstring wide_arg(const wchar_t*) { return L""; }
static std::string read_beside_exe(const wchar_t*) { return {}; }
#endif

int main(int argc, char* argv[]) {
    std::signal(SIGINT, signal_handler);
    std::signal(SIGTERM, signal_handler);

    // Silence noisy OpenCV internal logs (they go to stderr and pollute our log)
    cv::utils::logging::setLogLevel(cv::utils::logging::LOG_LEVEL_ERROR);

    // --version: used by the UI's system check to test that the exe and all
    // its DLLs load (WDAC, missing VC++ runtime...). Exit before logging.
    for (int i = 1; i < argc; i++)
        if (std::string(argv[i]) == "--version") { std::printf("palmsync-eye 0.4.0\n"); return 0; }

    eye_log_init(wide_arg(L"--log-dir"));
    EYE_LOG("[Eye] ==================== PalmSync Eye started ====================\n");

    EYE_LOG("[Eye] PalmSync Eye v0.4.0 starting\n");
    EYE_LOG("[Eye] OpenCV %s\n", CV_VERSION);

    // Log which camera backends this OpenCV build actually has. If neither
    // MSMF nor DSHOW appear here, the camera CANNOT open on Windows — the
    // OpenCV build is missing the msmf/dshow features (rebuild via vcpkg).
    {
        std::string cams;
        for (auto b : cv::videoio_registry::getCameraBackends())
            cams += cv::videoio_registry::getBackendName(b) + " ";
        EYE_LOG("[Eye] Camera backends available: %s\n",
                cams.empty() ? "(NONE! rebuild OpenCV with msmf+dshow)" : cams.c_str());
    }

    // ── Configuration ─────────────────────────────────────
    int camera_id = -1;  // -1 = auto-probe
    int fps = 30;
    std::string ipc_addr = get_env("PALMSYNC_IPC_ADDR", "127.0.0.1");
    int ipc_port = std::atoi(get_env("PALMSYNC_IPC_PORT", "12345").c_str());
    float threshold = 0.40f;
    bool show_window = false;
    bool probe = false;     // open the camera, report, exit (system check)
    bool verbose = false;   // log detection stats every 2 s (debug mode)
    std::string preview_pipe_name;

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
        } else if (arg == "--preview-pipe" && i + 1 < argc) {
            preview_pipe_name = argv[++i];
        } else if (arg == "--log-dir" && i + 1 < argc) {
            ++i;  // already handled via the wide command line
        } else if (arg == "--probe") {
            probe = true;
        } else if (arg == "--verbose") {
            verbose = true;
        } else if (arg == "--window") {
            show_window = true;
        } else if (arg == "--no-window") {
            show_window = false;
        } else if (arg == "--help") {
            std::printf("PalmSync Eye - Hand Gesture Detection\n\n");
            std::printf("Show an OPEN hand, then close it into a FIST  = GRAB\n");
            std::printf("Show a FIST, then open your hand             = DROP\n\n");
            std::printf("Options:\n");
            std::printf("  --camera <id>          Camera index to try first (default: auto-probe)\n");
            std::printf("  --fps <n>              Target FPS (default: 30)\n");
            std::printf("  --port <n>             IPC UDP port (default: 12345)\n");
            std::printf("  --threshold <0-1>      Fist/open boundary (default: 0.40)\n");
            std::printf("  --preview-pipe <name>  Send JPEG frames to named pipe (dev mode)\n");
            std::printf("  --window               Show live preview window (debug)\n");
            std::printf("  --log-dir <dir>        Folder for palmsync-eye.log\n");
            std::printf("  --probe                Only test that a camera opens, then exit (0 = ok)\n");
            std::printf("  --verbose              Log detection stats every 2 s\n");
            std::printf("  --help                 Show this help\n");
            return 0;
        }
    }

    EYE_LOG("[Eye] Config: camera_id=%d fps=%d ipc=%s:%d threshold=%.2f\n",
            camera_id, fps, ipc_addr.c_str(), ipc_port, threshold);

    HandDetector detector;
    if (detector.load_face_model(read_beside_exe(L"haarcascade_frontalface_default.xml")))
        EYE_LOG("[Eye] Face model loaded — faces are masked out of the hand search\n");
    else
        EYE_LOG("[Eye] Face model not found — hand lock needs an OPEN hand to start\n");
    if (detector.load_profile_model(read_beside_exe(L"haarcascade_profileface.xml")))
        EYE_LOG("[Eye] Profile face model loaded — turned heads are masked too\n");

    // ── Camera ────────────────────────────────────────────
    CameraManager camera(camera_id, fps);
    if (!camera.initialize()) {
        EYE_LOG("[Eye] Camera init failed — is a webcam connected?\n");
        if (probe) std::printf("PROBE_FAIL no camera could be opened\n");
        eye_log_close();
        return probe ? 2 : 1;
    }

    if (probe) {
        cv::Mat f;
        bool frame_ok = camera.capture_frame(f);
        EYE_LOG("[Eye] Probe: camera %d via %s%s, frame %s\n", camera.index(),
                camera.backend().c_str(), camera.monochrome() ? " (monochrome/IR)" : "",
                frame_ok ? "ok" : "FAILED");
        std::printf("PROBE_OK index=%d backend=%s mono=%d frame=%d face_model=%d\n", camera.index(),
                    camera.backend().c_str(), camera.monochrome() ? 1 : 0, frame_ok ? 1 : 0,
                    detector.has_face_model() ? 1 : 0);
        camera.release();
        eye_log_close();
        return frame_ok ? 0 : 3;
    }

    // ── Preview pipe (shown in the tray UI) ───────────────
#ifdef _WIN32
    g_preview_name = preview_pipe_name;
#endif

    // ── IPC Sender ────────────────────────────────────────
    IpcSender ipc(ipc_addr, ipc_port);
    if (!ipc.initialize()) {
        EYE_LOG("[Eye] IPC init failed — exiting\n");
        eye_log_close();
        return 1;
    }


    GestureConfig gcfg;
    gcfg.fist_threshold = threshold - 0.08f;
    gcfg.open_threshold = threshold + 0.12f;
    GestureFSM fsm(gcfg);

    std::string last_gesture;
    auto last_gesture_time = std::chrono::steady_clock::now();

    fsm.set_callback([&](GestureEvent event) {
        if (event == GestureEvent::Grab) {
            EYE_LOG("[Eye] >>> GRAB (open -> fist)\n");
            ipc.send_grab(0.95);
            last_gesture = "GRAB";
        } else if (event == GestureEvent::Drop) {
            EYE_LOG("[Eye] >>> DROP (fist -> open)\n");
            ipc.send_drop(0.95);
            last_gesture = "DROP";
        }
        last_gesture_time = std::chrono::steady_clock::now();
    });

    const char* window = "PalmSync Eye";
    if (show_window) cv::namedWindow(window, cv::WINDOW_AUTOSIZE);

    EYE_LOG("[Eye] Running. OPEN->FIST = GRAB, FIST->OPEN = DROP.\n");

    const auto t0 = std::chrono::steady_clock::now();
    int64_t stats_at = 0;
    int stats_frames = 0, stats_hands = 0;
    cv::Mat frame;
    float smoothed = 1.0f;
    bool have_hand = false;

    while (running) {
        if (!camera.capture_frame(frame)) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
            continue;
        }

        cv::flip(frame, frame, 1);

        const bool preview = preview_pipe_active();
        const bool draw = show_window || preview;
        const int64_t now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now() - t0).count();

        HandObservation obs;
        // Only the LOCKED hand is followed (never the biggest skin blob = face).
        bool found = detector.track(frame, obs, draw ? &frame : nullptr);
        {
            std::string ev = detector.take_event();
            if (!ev.empty() && (verbose || ev.rfind("[debug] ", 0) != 0))
                EYE_LOG("%s\n", ev.c_str());
        }

        if (found) {
            smoothed = have_hand ? (0.6f * smoothed + 0.4f * obs.openness) : obs.openness;
            have_hand = true;
        } else {
            have_hand = false;
        }
        fsm.update(found, smoothed, obs.cx, obs.cy, now_ms);

        if (verbose) {
            ++stats_frames;
            if (found) ++stats_hands;
            if (now_ms - stats_at >= 2000) {
                const char* pose = !found ? "none"
                    : fsm.classify(smoothed) == HandPose::Fist ? "fist"
                    : fsm.classify(smoothed) == HandPose::Open ? "open" : "unknown";
                EYE_LOG("[debug] %.1f fps, hand in %d/%d frames, openness=%.2f fingers=%d gaps=%d pose=%s armed=%d locked=%d\n",
                        stats_frames * 1000.0 / std::max<int64_t>(1, now_ms - stats_at),
                        stats_hands, stats_frames, smoothed, obs.fingers, obs.finger_gaps, pose,
                        fsm.armed() ? 1 : 0, detector.locked() ? 1 : 0);
                stats_at = now_ms;
                stats_frames = stats_hands = 0;
            }
        }

        if (draw) {
            const char* hand_state = "no hand";
            if (found) {
                switch (fsm.classify(smoothed)) {
                    case HandPose::Fist: hand_state = "FIST"; break;
                    case HandPose::Open: hand_state = "OPEN"; break;
                    default:             hand_state = "..."; break;
                }
            }
            cv::putText(frame, cv::format("%s  %.2f%s", hand_state, smoothed,
                        fsm.armed() ? "" : "  (wait)"),
                        {12, 28}, cv::FONT_HERSHEY_SIMPLEX, 0.7,
                        {255, 255, 255}, 2);
            cv::putText(frame, "OPEN->FIST = GRAB   FIST->OPEN = DROP",
                        {12, frame.rows - 16}, cv::FONT_HERSHEY_SIMPLEX, 0.55,
                        {200, 200, 200}, 2);

            auto since = std::chrono::duration_cast<std::chrono::milliseconds>(
                std::chrono::steady_clock::now() - last_gesture_time).count();
            if (!last_gesture.empty() && since < 1200) {
                cv::putText(frame, last_gesture, {frame.cols / 2 - 90, frame.rows / 2},
                            cv::FONT_HERSHEY_SIMPLEX, 2.0,
                            last_gesture == "GRAB" ? cv::Scalar(0, 220, 0)
                                                   : cv::Scalar(0, 160, 255), 4);
            }
        }

        if (preview) preview_pipe_write(frame);

        if (show_window) {
            cv::imshow(window, frame);
            int key = cv::waitKey(1);
            if (key == 27) running = false;
            if (cv::getWindowProperty(window, cv::WND_PROP_VISIBLE) < 1)
                running = false;
        }
        // No extra sleep: capture_frame() already blocks until the camera
        // delivers the next frame; sleeping on top of that halved the FPS.
    }

    EYE_LOG("[Eye] Shutting down\n");
    if (show_window) cv::destroyAllWindows();
    camera.release();
    ipc.shutdown();
#ifdef _WIN32
    if (g_preview_pipe != INVALID_HANDLE_VALUE) CloseHandle(g_preview_pipe);
#endif
    eye_log_close();
    return 0;
}
