#include "camera_manager.hpp"
#include "hand_backend.hpp"
#include "hand_detector.hpp"
#include "landmark_hand_detector.hpp"
#include "gesture_fsm.hpp"
#include "test_guide.hpp"
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
#include <memory>

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

// Big, high-contrast cue overlay for guided validation runs (--guide). Text is
// ASCII because OpenCV's Hershey fonts have no Turkish glyphs.
static void draw_centered(cv::Mat& img, const std::string& text, int baseline_y,
                          double scale, const cv::Scalar& color, int thickness) {
    int base = 0;
    const cv::Size size = cv::getTextSize(text, cv::FONT_HERSHEY_SIMPLEX, scale, thickness, &base);
    cv::putText(img, text, {std::max(8, (img.cols - size.width) / 2), baseline_y},
                cv::FONT_HERSHEY_SIMPLEX, scale, color, thickness, cv::LINE_AA);
}

static void darken(cv::Mat& img, const cv::Rect& area) {
    cv::Mat roi = img(area & cv::Rect(0, 0, img.cols, img.rows));
    roi.convertTo(roi, -1, 0.3, 0);
}

// The guide overlay is laid out for 960x720. Shrink it when the work area is
// smaller so the hint bar at the bottom never hides behind the taskbar.
static double guide_fit_scale() {
#ifdef _WIN32
    RECT work{};
    if (SystemParametersInfoW(SPI_GETWORKAREA, 0, &work, 0)) {
        const double w = work.right - work.left;
        const double h = work.bottom - work.top - 48;  // title bar
        return std::max(0.5, std::min(1.0, std::min(w / 960.0, h / 720.0)));
    }
#endif
    return 1.0;
}

static void draw_guide(cv::Mat& img, const TestGuide& guide, int64_t now_ms,
                       const std::string& title, bool hand, HandPose pose,
                       bool armed, float openness) {
    const int w = img.cols, h = img.rows;
    darken(img, {0, 0, w, 150});
    darken(img, {0, h - 80, w, 80});

    cv::Scalar color(255, 255, 255);
    switch (guide.step()) {
        case GuideStep::CueFist:  color = {0, 150, 255}; break;   // orange
        case GuideStep::CueOpen:
        case GuideStep::ShowOpen: color = {80, 230, 80}; break;   // green
        case GuideStep::HoldFist:
        case GuideStep::HoldOpen: color = {0, 230, 255}; break;   // yellow
        case GuideStep::Done:     color = {255, 220, 0}; break;   // cyan
        default: break;
    }

    if (!title.empty())
        cv::putText(img, title, {16, 32}, cv::FONT_HERSHEY_SIMPLEX, 0.75,
                    {220, 220, 220}, 2, cv::LINE_AA);
    if (guide.reps() > 0 && guide.step() != GuideStep::Empty) {
        const std::string counter = cv::format("Tekrar %d/%d", std::min(guide.rep() + 1, guide.reps()),
                                               guide.reps());
        int base = 0;
        const cv::Size size = cv::getTextSize(counter, cv::FONT_HERSHEY_SIMPLEX, 0.85, 2, &base);
        cv::putText(img, counter, {w - size.width - 16, 32}, cv::FONT_HERSHEY_SIMPLEX, 0.85,
                    {255, 255, 255}, 2, cv::LINE_AA);
    }

    draw_centered(img, guide.headline(now_ms), 95, 1.7, color, 4);

    const float progress = guide.progress(now_ms);
    if (progress >= 0.0f) {
        cv::rectangle(img, {16, 118, w - 32, 14}, {90, 90, 90}, cv::FILLED);
        cv::rectangle(img, {16, 118, static_cast<int>((w - 32) * progress), 14}, color, cv::FILLED);
    }

    const std::string hint = guide.hint(hand, pose, armed);
    if (!hint.empty())
        draw_centered(img, hint, h - 30, 0.95, hand || guide.step() == GuideStep::Empty
                      || guide.step() == GuideStep::Done || guide.step() == GuideStep::Countdown
                          ? cv::Scalar(255, 255, 255) : cv::Scalar(80, 80, 255), 2);

    const char* pose_text = !hand ? "yok" : pose == HandPose::Open ? "ACIK"
                          : pose == HandPose::Fist ? "YUMRUK" : "?";
    cv::putText(img, cv::format("el: %s  aciklik %.2f%s", pose_text, openness,
                                armed ? "" : "  (kilitli)"),
                {16, h - 92}, cv::FONT_HERSHEY_SIMPLEX, 0.6, {255, 255, 255}, 2, cv::LINE_AA);
}

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
        if (std::string(argv[i]) == "--version") { std::printf("palmsync-eye %s\n", PALMSYNC_VERSION); return 0; }

    eye_log_init(wide_arg(L"--log-dir"));
    EYE_LOG("[Eye] ==================== PalmSync Eye started ====================\n");

    EYE_LOG("[Eye] PalmSync Eye v%s starting\n", PALMSYNC_VERSION);
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
    bool threshold_set = false;  // --threshold also overrides the landmark band
    bool show_window = false;
    bool probe = false;     // open the camera, report, exit (system check)
    bool verbose = false;   // log detection stats every 2 s (debug mode)
    std::string preview_pipe_name;
    std::string detector_name = get_env("PALMSYNC_HAND_BACKEND", "auto");
    bool metrics = false;   // per-frame CSV for real-camera validation runs
    GuideConfig guide_cfg;  // --guide / --guide-empty: on-screen validation script
    guide_cfg.reps = 0;
    std::string guide_title;

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
            threshold_set = true;
        } else if (arg == "--preview-pipe" && i + 1 < argc) {
            preview_pipe_name = argv[++i];
        } else if (arg == "--detector" && i + 1 < argc) {
            detector_name = argv[++i];
        } else if (arg == "--log-dir" && i + 1 < argc) {
            ++i;  // already handled via the wide command line
        } else if (arg == "--metrics" && i + 1 < argc) {
            ++i;  // path read via the wide command line below
            metrics = true;
        } else if (arg == "--guide" && i + 1 < argc) {
            guide_cfg.reps = std::max(0, std::atoi(argv[++i]));
        } else if (arg == "--guide-empty" && i + 1 < argc) {
            guide_cfg.empty_seconds = std::max(0, std::atoi(argv[++i]));
        } else if (arg == "--guide-title" && i + 1 < argc) {
            guide_title = argv[++i];
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
            std::printf("  --detector <mode>      auto, landmark or classic (default: auto)\n");
            std::printf("  --preview-pipe <name>  Send JPEG frames to named pipe (dev mode)\n");
            std::printf("  --window               Show live preview window (debug)\n");
            std::printf("  --log-dir <dir>        Folder for palmsync-eye.log\n");
            std::printf("  --metrics <file.csv>   Write per-frame detection metrics (validation runs)\n");
            std::printf("  --guide <n>            On-screen guided test: n open->fist->open reps\n");
            std::printf("                         (opens the window, sends no UDP events)\n");
            std::printf("  --guide-empty <s>      Guided empty-scene test for s seconds\n");
            std::printf("  --guide-title <text>   Scenario name shown during a guided test\n");
            std::printf("  --probe                Only test that a camera opens, then exit (0 = ok)\n");
            std::printf("  --verbose              Log detection stats every 2 s\n");
            std::printf("  --help                 Show this help\n");
            return 0;
        }
    }

    if (guide_cfg.empty_seconds > 0) guide_cfg.reps = 0;
    const bool guided = guide_cfg.reps > 0 || guide_cfg.empty_seconds > 0;
    if (guided) show_window = true;

    HandBackend requested_backend;
    if (!parse_hand_backend(detector_name, requested_backend)) {
        EYE_LOG("[Eye] Invalid detector '%s' (expected auto, landmark or classic)\n",
                detector_name.c_str());
        std::fprintf(stderr, "Invalid --detector value: %s\n", detector_name.c_str());
        eye_log_close();
        return 4;
    }

    std::unique_ptr<LandmarkHandDetector> landmark_detector;
    bool landmark_available = false;
    if (requested_backend != HandBackend::Classic) {
        landmark_detector = std::make_unique<LandmarkHandDetector>(
            read_beside_exe(L"models\\palm_detection_mediapipe_2023feb.onnx"),
            read_beside_exe(L"models\\handpose_estimation_mediapipe_2023feb.onnx"));
        landmark_available = landmark_detector->ready();
        if (!landmark_available)
            EYE_LOG("[Eye] Landmark detector unavailable: %s\n",
                    landmark_detector->error().c_str());
    }
    const HandBackendResolution backend =
        resolve_hand_backend(requested_backend, landmark_available);
    if (!backend.available) {
        EYE_LOG("[Eye] Detector '%s' unavailable: %s\n",
                hand_backend_name(requested_backend), backend.reason.c_str());
        std::fprintf(stderr, "Detector '%s' unavailable: %s\n",
                     hand_backend_name(requested_backend), backend.reason.c_str());
        eye_log_close();
        return 4;
    }

    EYE_LOG("[Eye] Config: camera_id=%d fps=%d ipc=%s:%d threshold=%.2f detector=%s\n",
            camera_id, fps, ipc_addr.c_str(), ipc_port, threshold,
            hand_backend_name(requested_backend));
    EYE_LOG("[Eye] Hand detector selected: %s (%s)\n",
            hand_backend_name(backend.selected), backend.reason.c_str());

    std::unique_ptr<HandDetector> classic_detector;
    IHandTracker* detector = nullptr;
    bool face_model_loaded = false;
    if (backend.selected == HandBackend::Landmark) {
        detector = landmark_detector.get();
        EYE_LOG("[Eye] Camera 2.0 landmark models loaded\n");
    } else {
        classic_detector = std::make_unique<HandDetector>();
        detector = classic_detector.get();
        face_model_loaded = classic_detector->load_face_model(
            read_beside_exe(L"haarcascade_frontalface_default.xml"));
        if (face_model_loaded)
            EYE_LOG("[Eye] Face model loaded — faces are masked out of the hand search\n");
        else
            EYE_LOG("[Eye] Face model not found — hand lock needs an OPEN hand to start\n");
        if (classic_detector->load_profile_model(
                read_beside_exe(L"haarcascade_profileface.xml")))
            EYE_LOG("[Eye] Profile face model loaded — turned heads are masked too\n");
    }

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
        std::printf("PROBE_OK index=%d backend=%s mono=%d frame=%d face_model=%d detector=%s\n", camera.index(),
                    camera.backend().c_str(), camera.monochrome() ? 1 : 0, frame_ok ? 1 : 0,
                    face_model_loaded ? 1 : 0, hand_backend_name(backend.selected));
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


    const GestureConfig gcfg = gesture_config_for(
        backend.selected == HandBackend::Landmark && !threshold_set, threshold);
    EYE_LOG("[Eye] Pose thresholds: fist < %.2f, open > %.2f\n",
            gcfg.fist_threshold, gcfg.open_threshold);
    GestureFSM fsm(gcfg);

    std::string last_gesture;
    std::string frame_event;  // gesture fired during the current frame, for --metrics
    auto last_gesture_time = std::chrono::steady_clock::now();

    fsm.set_callback([&](GestureEvent event) {
        // A guided test must never capture or restore a real workspace.
        if (event == GestureEvent::Grab) {
            EYE_LOG("[Eye] >>> GRAB (open -> fist)%s\n", guided ? " [test, not sent]" : "");
            if (!guided) ipc.send_grab(0.95);
            last_gesture = "GRAB";
        } else if (event == GestureEvent::Drop) {
            EYE_LOG("[Eye] >>> DROP (fist -> open)%s\n", guided ? " [test, not sent]" : "");
            if (!guided) ipc.send_drop(0.95);
            last_gesture = "DROP";
        }
        frame_event = last_gesture;
        last_gesture_time = std::chrono::steady_clock::now();
    });

    // One row per processed frame. raw_pose classifies the unsmoothed
    // openness so analyze-metrics.ps1 can measure how long the pipeline takes
    // from the first frame showing the new pose to the emitted gesture.
    FILE* metrics_file = nullptr;
    if (metrics) {
#ifdef _WIN32
        metrics_file = _wfopen(wide_arg(L"--metrics").c_str(), L"w");
#else
        for (int i = 1; i + 1 < argc; ++i)
            if (std::string(argv[i]) == "--metrics") metrics_file = std::fopen(argv[i + 1], "w");
#endif
        if (metrics_file) {
            std::fprintf(metrics_file, "t_ms,found,confidence,openness,smoothed,fingers,"
                                       "raw_pose,pose,track_ms,event,cue,cx,cy\n");
            EYE_LOG("[Eye] Writing per-frame metrics (detector=%s)\n",
                    hand_backend_name(backend.selected));
        } else {
            EYE_LOG("[Eye] Could not open --metrics file; metrics disabled\n");
        }
    }
    auto pose_name = [&](bool present, float openness) {
        if (!present) return "none";
        switch (fsm.classify(openness)) {
            case HandPose::Fist: return "fist";
            case HandPose::Open: return "open";
            default:             return "unknown";
        }
    };

    std::unique_ptr<TestGuide> guide;
    if (guided) {
        guide = std::make_unique<TestGuide>(guide_cfg);
        if (guide_cfg.empty_seconds > 0)
            EYE_LOG("[Eye] Guided empty-scene test: %d s '%s' (UDP events disabled)\n",
                    guide_cfg.empty_seconds, guide_title.c_str());
        else
            EYE_LOG("[Eye] Guided gesture test: %d reps '%s' (UDP events disabled)\n",
                    guide_cfg.reps, guide_title.c_str());
    }

    const char* window = "PalmSync Eye";
    double guide_scale = 1.0;
    if (guide) {
#ifdef _WIN32
        // Without this, 125%+ display scaling blows the window past the screen.
        SetProcessDPIAware();
#endif
        guide_scale = guide_fit_scale();
    }
    if (show_window) cv::namedWindow(window, cv::WINDOW_AUTOSIZE);
    if (guide) cv::moveWindow(window, 0, 0);

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
        const auto track_started = std::chrono::steady_clock::now();
        bool found = detector->track(frame, obs, draw ? &frame : nullptr);
        const double track_ms = std::chrono::duration<double, std::milli>(
            std::chrono::steady_clock::now() - track_started).count();
        {
            std::string ev = detector->take_event();
            if (!ev.empty() && (verbose || ev.rfind("[debug] ", 0) != 0))
                EYE_LOG("%s\n", ev.c_str());
        }

        if (found) {
            // Prefer the current frame so a real open<->fist transition is
            // visible to the FSM within a few frames instead of almost a
            // second later. The FSM itself still filters short flicker.
            smoothed = have_hand ? (0.15f * smoothed + 0.85f * obs.openness) : obs.openness;
            have_hand = true;
        } else {
            have_hand = false;
        }
        frame_event.clear();
        fsm.update(found, smoothed, obs.cx, obs.cy, now_ms);

        // The cue active when this frame's event fired decides whether the
        // event answered it, so record it before the guide advances.
        const std::string cue = guide ? guide->step_name() : "";
        std::string marker;
        if (guide) {
            marker = guide->update(now_ms, found, fsm.pose(), fsm.armed(), frame_event);
            if (!marker.empty()) EYE_LOG("[Eye] Guide: %s\n", marker.c_str());
            if (guide->finished(now_ms)) running = false;
        }

        if (metrics_file) {
            std::fprintf(metrics_file, "%lld,%d,%.3f,%.3f,%.3f,%d,%s,%s,%.2f,%s,%s,%.3f,%.3f\n",
                         static_cast<long long>(now_ms), found ? 1 : 0, obs.confidence,
                         obs.openness, smoothed, obs.fingers, pose_name(found, obs.openness),
                         pose_name(found, smoothed), track_ms,
                         frame_event.empty() ? marker.c_str() : frame_event.c_str(), cue.c_str(),
                         obs.cx, obs.cy);
            std::fflush(metrics_file);  // keep rows if the UI kills the process
        }

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
                        fsm.armed() ? 1 : 0, detector->locked() ? 1 : 0);
                stats_at = now_ms;
                stats_frames = stats_hands = 0;
            }
        }

        cv::Mat shown = frame;
        if (guide && show_window) {
            // Upscale so the cues are readable from 1.5 m away.
            cv::resize(frame, shown, cv::Size(960, 720), 0, 0, cv::INTER_LINEAR);
            draw_guide(shown, *guide, now_ms, guide_title, found, fsm.pose(),
                       fsm.armed(), smoothed);
            if (guide_scale < 1.0)
                cv::resize(shown, shown, cv::Size(), guide_scale, guide_scale, cv::INTER_AREA);
        } else if (draw) {
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
            cv::imshow(window, shown);
            int key = cv::waitKey(1);
            if (key == 27) running = false;
            if (cv::getWindowProperty(window, cv::WND_PROP_VISIBLE) < 1)
                running = false;
        }
        // No extra sleep: capture_frame() already blocks until the camera
        // delivers the next frame; sleeping on top of that halved the FPS.
    }

    if (guide) {
        EYE_LOG("[Eye] Guide result (%s): %s\n",
                guide->step() == GuideStep::Done ? "completed" : "aborted",
                guide->summary().c_str());
        std::printf("GUIDE_RESULT completed=%d %s\n", guide->step() == GuideStep::Done ? 1 : 0,
                    guide->summary().c_str());
    }
    EYE_LOG("[Eye] Shutting down\n");
    if (show_window) cv::destroyAllWindows();
    if (metrics_file) std::fclose(metrics_file);
    camera.release();
    ipc.shutdown();
#ifdef _WIN32
    if (g_preview_pipe != INVALID_HANDLE_VALUE) CloseHandle(g_preview_pipe);
#endif
    eye_log_close();
    return 0;
}
