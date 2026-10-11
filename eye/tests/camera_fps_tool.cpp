// palmsync-camera-fps [camera index]
//
// Opens the camera with a few setting combinations and reports the frame
// rate the driver really delivers. Webcams silently halve their FPS in dim
// light (auto-exposure lengthens the shutter), which directly adds gesture
// latency, so this shows which settings keep 30 FPS on a given camera.
#include <opencv2/core.hpp>
#include <opencv2/videoio.hpp>
#include <opencv2/core/utils/logger.hpp>

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <vector>

struct Mode {
    const char* name;
    bool mjpg;
    double exposure;  // 0 = leave auto exposure alone
};

int main(int argc, char** argv) {
    cv::utils::logging::setLogLevel(cv::utils::logging::LOG_LEVEL_ERROR);
    const int index = argc > 1 ? std::atoi(argv[1]) : 0;
    const Mode modes[] = {
        {"default (YUY2, auto exposure)", false, 0},
        {"MJPG, auto exposure", true, 0},
        {"MJPG, exposure -6 (~1/64 s)", true, -6},
        {"MJPG, exposure -5 (~1/32 s)", true, -5},
    };
    for (const auto& m : modes) {
        cv::VideoCapture cap(index, cv::CAP_DSHOW);
        if (!cap.isOpened()) {
            std::printf("camera %d could not be opened\n", index);
            return 1;
        }
        if (m.mjpg) cap.set(cv::CAP_PROP_FOURCC, cv::VideoWriter::fourcc('M', 'J', 'P', 'G'));
        cap.set(cv::CAP_PROP_FRAME_WIDTH, 640);
        cap.set(cv::CAP_PROP_FRAME_HEIGHT, 480);
        cap.set(cv::CAP_PROP_FPS, 30);
        cap.set(cv::CAP_PROP_BUFFERSIZE, 1);
        if (m.exposure != 0) {
            cap.set(cv::CAP_PROP_AUTO_EXPOSURE, 0.25);  // DirectShow: manual
            cap.set(cv::CAP_PROP_EXPOSURE, m.exposure);
        }

        cv::Mat frame;
        for (int i = 0; i < 20; ++i) cap.read(frame);  // let exposure settle
        std::vector<double> gaps;
        double brightness = 0;
        auto last = std::chrono::steady_clock::now();
        const auto start = last;
        int frames = 0;
        while (std::chrono::steady_clock::now() - start < std::chrono::seconds(4)) {
            if (!cap.read(frame) || frame.empty()) continue;
            const auto now = std::chrono::steady_clock::now();
            gaps.push_back(std::chrono::duration<double, std::milli>(now - last).count());
            last = now;
            ++frames;
            const cv::Scalar mean = cv::mean(frame);
            brightness = (mean[0] + mean[1] + mean[2]) / 3.0;
        }
        const double secs = std::chrono::duration<double>(last - start).count();
        std::sort(gaps.begin(), gaps.end());
        const double p50 = gaps.empty() ? 0 : gaps[gaps.size() / 2];
        const double p95 = gaps.empty() ? 0 : gaps[gaps.size() * 95 / 100];
        std::printf("%-32s %5.1f FPS  frame gap p50 %5.1f ms p95 %5.1f ms  brightness %3.0f  (driver says %.0f fps)\n",
                    m.name, frames / std::max(0.001, secs), p50, p95, brightness,
                    cap.get(cv::CAP_PROP_FPS));
        cap.release();
    }
    return 0;
}
