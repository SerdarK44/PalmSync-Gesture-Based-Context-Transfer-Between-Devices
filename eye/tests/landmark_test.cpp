#undef NDEBUG
#include "landmark_hand_detector.hpp"

#include <opencv2/core.hpp>
#include <opencv2/imgcodecs.hpp>

#include <array>
#include <cassert>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <fstream>
#include <iterator>
#include <string>

static std::string read_file(const std::string& path) {
    std::ifstream stream(path, std::ios::binary);
    return {std::istreambuf_iterator<char>(stream), std::istreambuf_iterator<char>()};
}

static std::array<cv::Point3f, 21> open_hand() {
    std::array<cv::Point3f, 21> p{};
    p[0] = {0, 4, 0};
    p[1] = {-0.4f, 3.3f, 0}; p[2] = {-0.9f, 2.8f, 0};
    p[3] = {-1.4f, 2.3f, 0}; p[4] = {-1.9f, 1.8f, 0};
    const float xs[] = {-1.5f, -0.5f, 0.5f, 1.5f};
    for (int finger = 0; finger < 4; ++finger) {
        const int i = 5 + finger * 4;
        p[i] = {xs[finger], 3, 0};
        p[i + 1] = {xs[finger], 2, 0};
        p[i + 2] = {xs[finger], 1, 0};
        p[i + 3] = {xs[finger], 0, 0};
    }
    return p;
}

static std::array<cv::Point3f, 21> fist() {
    auto p = open_hand();
    p[1] = {-0.4f, 3.3f, 0}; p[2] = {-0.8f, 3.0f, 0};
    p[3] = {-0.4f, 3.3f, 0}; p[4] = {0.0f, 3.6f, 0};
    const float xs[] = {-1.5f, -0.5f, 0.5f, 1.5f};
    for (int finger = 0; finger < 4; ++finger) {
        const int i = 5 + finger * 4;
        p[i] = {xs[finger], 3, 0};
        p[i + 1] = {xs[finger], 2.5f, 0};
        p[i + 2] = {xs[finger], 3.0f, 0};
        p[i + 3] = {xs[finger], 3.5f, 0};
    }
    return p;
}

// Straight thumb laid across the palm towards the pinky side: every thumb
// joint angle reads as extended, but the thumb is closed.
static std::array<cv::Point3f, 21> thumb_across_palm() {
    auto p = open_hand();
    p[1] = {-0.4f, 3.3f, 0}; p[2] = {0.1f, 3.0f, 0};
    p[3] = {0.6f, 2.7f, 0};  p[4] = {1.1f, 2.4f, 0};
    return p;
}

// The other hand (or the same hand seen from the back) is a mirror image.
static std::array<cv::Point3f, 21> mirrored(std::array<cv::Point3f, 21> p) {
    for (auto& point : p) point.x = -point.x;
    return p;
}

int main(int argc, char** argv) {
    int fingers = 0;
    const float open = LandmarkHandDetector::measure_openness(open_hand(), fingers);
    assert(open > 0.9f && fingers == 5);
    const float closed = LandmarkHandDetector::measure_openness(fist(), fingers);
    assert(closed < 0.15f && fingers == 0);
    std::printf("geometry open=%.2f fist=%.2f\n", open, closed);

    const float tucked = LandmarkHandDetector::measure_openness(thumb_across_palm(), fingers);
    assert(fingers == 4 && tucked < 0.85f);
    const float other_open = LandmarkHandDetector::measure_openness(mirrored(open_hand()), fingers);
    assert(fingers == 5 && std::abs(other_open - open) < 1e-5f);
    const float other_tucked = LandmarkHandDetector::measure_openness(mirrored(thumb_across_palm()), fingers);
    assert(fingers == 4 && std::abs(other_tucked - tucked) < 1e-5f);
    std::printf("thumb geometry tucked=%.2f mirrored open=%.2f\n", tucked, other_open);

    const std::string model_dir = PALMSYNC_TEST_MODEL_DIR;
    LandmarkHandDetector detector(
        read_file(model_dir + "/palm_detection_mediapipe_2023feb.onnx"),
        read_file(model_dir + "/handpose_estimation_mediapipe_2023feb.onnx"));
    assert(detector.ready());
    HandObservation obs;
    cv::Mat blank(480, 640, CV_8UC3, cv::Scalar(0, 0, 0));
    assert(!detector.track(blank, obs));
    std::printf("PASS: landmark models load and blank frame is rejected\n");

    // Optional manual/CI fixture path: run the complete PalmDet -> HandPose
    // pipeline and print the resulting geometry.
    if (argc > 1) {
        cv::Mat fixture = cv::imread(argv[1]);
        assert(!fixture.empty());
        if (argc == 7) {
            const cv::Rect requested(std::stoi(argv[3]), std::stoi(argv[4]),
                                     std::stoi(argv[5]), std::stoi(argv[6]));
            const cv::Rect image_bounds(0, 0, fixture.cols, fixture.rows);
            const cv::Rect crop = requested & image_bounds;
            assert(crop.width > 0 && crop.height > 0);
            fixture = fixture(crop).clone();
        }
        cv::Mat overlay = fixture.clone();
        const bool found = detector.track(fixture, obs, &overlay);
        std::printf("fixture found=%d confidence=%.3f openness=%.3f fingers=%d\n",
                    found ? 1 : 0, obs.confidence, obs.openness, obs.fingers);
        assert(found && obs.has_landmarks);
        const auto started = std::chrono::steady_clock::now();
        for (int i = 0; i < 20; ++i) assert(detector.track(fixture, obs));
        const double elapsed_ms = std::chrono::duration<double, std::milli>(
            std::chrono::steady_clock::now() - started).count();
        std::printf("tracked ROI average=%.2f ms/frame\n", elapsed_ms / 20.0);
        if (argc > 2) assert(cv::imwrite(argv[2], overlay));
    }
    return 0;
}

