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

    double c = colorfulness(test);
    is_mono = c < 1.0;
    EYE_LOG("[Eye]   -> %dx%d, colorfulness=%.2f%s\n", test.cols, test.rows, c,
            is_mono ? " (monochrome/IR camera)" : "");
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

            if (is_mono) {
                // Keep looking for a colour camera, remember this as fallback.
                if (mono_idx < 0) { mono_idx = idx; mono_api = b.api; mono_name = b.name; }
                cap_.release();
                continue;
            }

            return finish(idx, b.name, false);
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
    EYE_LOG("[Eye] Camera %d opened via %s  size=%.0fx%.0f fps=%.1f\n",
            idx, name,
            cap_.get(cv::CAP_PROP_FRAME_WIDTH),
            cap_.get(cv::CAP_PROP_FRAME_HEIGHT),
            cap_.get(cv::CAP_PROP_FPS));
    return true;
}

bool CameraManager::capture_frame(cv::Mat& frame) {
    if (!initialized_) return false;
    return cap_.read(frame) && !frame.empty();
}

void CameraManager::release() {
    if (initialized_) {
        cap_.release();
        initialized_ = false;
    }
}

bool CameraManager::is_available() const {
    return initialized_ && cap_.isOpened();
}
