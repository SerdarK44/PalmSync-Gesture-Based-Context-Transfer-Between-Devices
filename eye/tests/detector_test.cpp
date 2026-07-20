// Deterministic test for the model-free HandDetector using synthetic,
// skin-coloured images (no camera needed). Verifies that a spread "hand"
// reads as more open than a compact "fist".

#include "hand_detector.hpp"

#include <opencv2/imgproc.hpp>

#include <cassert>
#include <cmath>
#include <cstdio>

// A BGR value that lands inside the YCrCb skin range the detector uses.
static const cv::Scalar kSkin(110, 140, 200);

static cv::Mat make_fist() {
    cv::Mat img(480, 640, CV_8UC3, cv::Scalar(0, 0, 0));
    cv::circle(img, {320, 240}, 90, kSkin, cv::FILLED);
    return img;
}

static cv::Mat make_open_hand() {
    cv::Mat img(480, 640, CV_8UC3, cv::Scalar(0, 0, 0));
    cv::Point center(320, 260);
    cv::circle(img, center, 70, kSkin, cv::FILLED);  // palm

    // Five fingers radiating upward, with gaps between them
    const double angles[] = {-100, -125, -90, -55, -80};
    for (double deg : angles) {
        double rad = deg * CV_PI / 180.0;
        cv::Point tip(center.x + static_cast<int>(150 * std::cos(rad)),
                      center.y + static_cast<int>(150 * std::sin(rad)));
        cv::line(img, center, tip, kSkin, 22);
    }
    return img;
}

int main() {
    HandDetector detector;

    float fist_open = -1, open_open = -1, conf = 0;
    bool f1 = detector.detect(make_fist(), fist_open, conf, nullptr);
    bool f2 = detector.detect(make_open_hand(), open_open, conf, nullptr);

    std::printf("fist detected=%d openness=%.2f\n", f1, fist_open);
    std::printf("open detected=%d openness=%.2f\n", f2, open_open);

    assert(f1 && "fist blob should be detected");
    assert(f2 && "open hand should be detected");
    assert(fist_open < open_open && "fist must read less open than a spread hand");
    assert(fist_open < 0.4f && "a solid blob should be well below the fist threshold");
    assert(open_open > fist_open + 0.2f && "clear separation between the two");

    std::printf("PASS: detector discriminates fist vs open hand\n");
    return 0;
}
