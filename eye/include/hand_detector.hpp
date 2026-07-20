#pragma once

#include <opencv2/core.hpp>

// Model-free hand openness detector.
//
// Instead of ML landmark models (which needed .onnx files that MediaPipe does
// not ship), this uses classic computer vision: skin-colour segmentation +
// the convexity-defect finger-counting technique. It reports how OPEN the hand
// is (0 = fist, 1 = fully spread), which the GestureFSM turns into GRAB/DROP.
class HandDetector {
public:
    explicit HandDetector(int min_area_percent = 3);

    // Analyse a frame. On success sets `openness` in [0,1] and returns true.
    // Returns false when no hand-sized skin region is found.
    // If `overlay` is non-null, draws the detected hand + metrics onto it.
    bool detect(const cv::Mat& frame, float& openness, float& confidence,
                cv::Mat* overlay = nullptr);

private:
    double min_area_ratio_;  // hand must cover at least this fraction of frame
};
