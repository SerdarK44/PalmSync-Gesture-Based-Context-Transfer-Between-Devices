#include "camera_manager.hpp"
#include "log.hpp"

#include <opencv2/imgproc.hpp>

#include <vector>

CameraManager::CameraManager(int camera_id, int fps)
    : camera_id_(camera_id), fps_(fps) {}

CameraManager::~CameraManager() {
    release();
}

namespace {

// Mean per-pixel channel difference. ~0 for IR / monochrome sensors (Windows
// Hello cameras), which deliver grey frames where skin detection never works.
double colorfulness(const cv::Mat& bgr) {
    if (bgr.channels() < 3) return 0.0;
    std::vector<cv::Mat> ch;
    cv::split(bgr, ch);
    cv::Mat d1, d2;
    cv::absdiff(ch[0], ch[1], d1);
    cv::absdiff(ch[1], ch[2], d2);
    return (cv::mean(d1)[0] + cv::mean(d2)[0]) / 2.0;
}

}  // namespace

bool CameraManager::try_open(int idx, int api, const char* name, bool& is_mono) {
    EYE_LOG("[Eye] Trying camera %d via %s...\n", idx, name);

    cap_.open(idx, api);
    if (!cap_.isOpened()) {
        EYE_LOG("[Eye]   -> open() returned false\n");
        cap_.release();
        return false;
    }

    cap_.set(cv::CAP_PROP_FRAME_WIDTH,  640);
    cap_.set(cv::CAP_PROP_FRAME_HEIGHT, 480);
    cap_.set(cv::CAP_PROP_FPS, fps_);

    // Some drivers report isOpened()=true but deliver no frames, and many
    // webcams send a few black frames while auto-exposure settles.
    cv::Mat test;
    bool got = false;
    for (int i = 0; i < 15; ++i) {
        if (cap_.read(test) && !test.empty()) {
            got = true;
            if (cv::mean(test)[0] + cv::mean(test)[1] + cv::mean(test)[2] > 15.0) break;
        }
    }
    if (!got) {
        EYE_LOG("[Eye]   -> no frames delivered — skipping\n");
        cap_.release();
        return false;
    }

    // A dark colour frame also has small channel differences (0.83 was
    // measured in a dim room), so the threshold scales with brightness. A
    // real IR/mono sensor stays near zero at any brightness.
    const double c = colorfulness(test);
    const cv::Scalar m = cv::mean(test);
    const double brightness = (m[0] + m[1] + m[2]) / 3.0;
    is_mono = c < 0.25 || (c < 1.0 && c < brightness * 0.01);
    EYE_LOG("[Eye]   -> %dx%d, colorfulness=%.2f brightness=%.0f%s\n", test.cols, test.rows,
            c, brightness, is_mono ? " (monochrome/IR camera)" : "");
    return true;
}

bool CameraManager::initialize() {
    struct Backend { int api; const char* name; };
    constexpr Backend backends[] = {
        { cv::CAP_DSHOW, "DirectShow" },
        { cv::CAP_MSMF,  "MSMF"       },
        { cv::CAP_ANY,   "Auto"       },
    };

    // Requested index first (if any), then probe a wide range.
    std::vector<int> indices;
    if (camera_id_ >= 0) indices.push_back(camera_id_);
    for (int i = 0; i < 8; ++i)
        if (i != camera_id_) indices.push_back(i);

    int mono_idx = -1, mono_api = 0;
    const char* mono_name = "";

    for (const auto& b : backends) {
        for (int idx : indices) {
            bool is_mono = false;
            if (!try_open(idx, b.api, b.name, is_mono)) continue;

            // An explicitly requested camera is used even if it looks grey.
            if (is_mono && idx != camera_id_) {
                // Keep looking for a colour camera, remember this as fallback.
                if (mono_idx < 0) { mono_idx = idx; mono_api = b.api; mono_name = b.name; }
                cap_.release();
                continue;
            }

            return finish(idx, b.name, is_mono);
        }
    }

    if (mono_idx >= 0) {
        bool is_mono = false;
        if (try_open(mono_idx, mono_api, mono_name, is_mono)) {
            EYE_LOG("[Eye] Only a monochrome camera was found — gestures may not work.\n");
            return finish(mono_idx, mono_name, true);
        }
    }

    EYE_LOG("[Eye] Camera init FAILED — tried DirectShow/MSMF/Auto on indices 0-7.\n");
    EYE_LOG("[Eye] Check: Settings -> Privacy & security -> Camera:\n");
    EYE_LOG("[Eye]   'Camera access', 'Let apps access', 'Let desktop apps access' must ALL be ON.\n");
    EYE_LOG("[Eye] Also make sure no other app (Teams, Zoom, OBS) has the camera open.\n");
    return false;
}

bool CameraManager::finish(int idx, const char* name, bool mono) {
    camera_id_   = idx;
    initialized_ = true;
    backend_     = name;
    mono_        = mono;
    // Ask the driver not to queue frames; the reader thread below keeps only
    // the newest one anyway (not every backend honours this property).
    cap_.set(cv::CAP_PROP_BUFFERSIZE, 1);
    EYE_LOG("[Eye] Camera %d opened via %s  size=%.0fx%.0f fps=%.1f\n",
            idx, name,
            cap_.get(cv::CAP_PROP_FRAME_WIDTH),
            cap_.get(cv::CAP_PROP_FRAME_HEIGHT),
            cap_.get(cv::CAP_PROP_FPS));
    stop_ = false;
    reader_ = std::thread(&CameraManager::reader_loop, this);
    return true;
}

// Reads frames as fast as the camera delivers them. Processing used to call
// read() itself, so whenever a frame took longer than the frame interval the
// driver's queue filled up and every gesture was seen a few frames late.
void CameraManager::reader_loop() {
    cv::Mat frame;
    while (!stop_) {
        if (!cap_.read(frame) || frame.empty()) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
            continue;
        }
        const auto now = std::chrono::steady_clock::now();
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (seq_ != consumed_) ++skipped_;
            cv::swap(latest_, frame);
            latest_at_ = now;
            ++seq_;
        }
        ready_.notify_one();
    }
}

bool CameraManager::capture_frame(cv::Mat& frame,
                                  std::chrono::steady_clock::time_point* captured_at) {
    if (!initialized_) return false;
    std::unique_lock<std::mutex> lock(mutex_);
    if (!ready_.wait_for(lock, std::chrono::seconds(1),
                         [this] { return seq_ != consumed_ || stop_; }) ||
        seq_ == consumed_) {
        return false;
    }
    // Hand the buffer over instead of copying; the reader writes into a
    // fresh Mat next time.
    frame = latest_;
    latest_ = cv::Mat();
    consumed_ = seq_;
    if (captured_at) *captured_at = latest_at_;
    return !frame.empty();
}

void CameraManager::release() {
    if (initialized_) {
        stop_ = true;
        ready_.notify_all();
        if (reader_.joinable()) reader_.join();
        cap_.release();
        initialized_ = false;
    }
}

bool CameraManager::is_available() const {
    return initialized_ && cap_.isOpened();
}
