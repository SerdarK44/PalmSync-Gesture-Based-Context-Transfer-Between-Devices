#pragma once

#include "hand_tracker.hpp"

#include <opencv2/dnn.hpp>

#include <array>
#include <string>
#include <vector>

// OpenCV DNN implementation of the MediaPipe-style PalmDet + HandPose
// pipeline. The models are loaded from verified ONNX bytes so Unicode install
// paths on Windows do not have to pass through OpenCV's narrow path APIs.
class LandmarkHandDetector final : public IHandTracker {
public:
    LandmarkHandDetector(const std::string& palm_onnx,
                         const std::string& handpose_onnx);

    bool ready() const { return ready_; }
    const std::string& error() const { return error_; }

    bool track(const cv::Mat& frame, HandObservation& obs,
               cv::Mat* overlay = nullptr) override;
    bool locked() const override { return locked_; }
    void reset() override;
    std::string take_event() override;

    // Kept public for deterministic geometry tests. Returns [0,1].
    static float measure_openness(const std::array<cv::Point3f, 21>& landmarks,
                                  int& extended_fingers);

private:
    struct Palm {
        cv::Rect2f box;
        std::array<cv::Point2f, 7> landmarks{};
        float confidence = 0.0f;
    };

    struct HandResult {
        std::array<cv::Point3f, 21> landmarks{};
        cv::Rect2f box;
        float confidence = 0.0f;
        float handedness = 0.5f;
    };

    bool detect_palm(const cv::Mat& frame, Palm& palm);
    bool estimate_hand(const cv::Mat& frame, const Palm& palm,
                       HandResult& hand);
    static Palm palm_from_hand(const HandResult& hand);
    void mark_missed();
    static std::vector<cv::Point2f> make_anchors();
    static void draw_hand(cv::Mat& image, const HandResult& hand,
                          float openness);

    cv::dnn::Net palm_net_;
    cv::dnn::Net handpose_net_;
    std::vector<cv::Point2f> anchors_;
    bool ready_ = false;
    bool locked_ = false;
    int lost_frames_ = 0;
    bool have_tracked_palm_ = false;
    Palm tracked_palm_;
    std::string error_;
    std::string event_;
};

