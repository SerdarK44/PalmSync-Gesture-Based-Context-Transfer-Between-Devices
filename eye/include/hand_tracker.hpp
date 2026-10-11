#pragma once

#include <opencv2/core.hpp>

#include <array>
#include <string>

// Backend-independent hand state consumed by the gesture pipeline. Landmark
// trackers can extend how these values are calculated without coupling the
// camera loop or GestureFSM to a particular detection implementation.
struct HandObservation {
    float openness = 0.0f;       // 0 = fist, 1 = fully spread
    float confidence = 0.0f;
    float cx = 0.0f, cy = 0.0f; // hand centre, normalised to [0,1]
    int finger_gaps = 0;
    int fingers = 0;
    bool has_landmarks = false;
    std::array<cv::Point3f, 21> landmarks{};
    float handedness = 0.5f;    // 0 = left, 1 = right
};

// Common boundary for the classic contour tracker and the Camera 2.0
// landmark tracker. Keeping the camera loop on this interface makes backend
// selection and a safe fallback possible without changing gesture delivery.
class IHandTracker {
public:
    virtual ~IHandTracker() = default;

    virtual bool track(const cv::Mat& frame, HandObservation& obs,
                       cv::Mat* overlay = nullptr) = 0;
    virtual bool locked() const = 0;
    virtual void reset() = 0;
    virtual std::string take_event() = 0;
};

