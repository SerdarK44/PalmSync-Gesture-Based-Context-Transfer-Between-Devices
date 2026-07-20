#include "hand_detector.hpp"

#include <opencv2/imgproc.hpp>

#include <algorithm>
#include <cmath>
#include <vector>

HandDetector::HandDetector(int min_area_percent)
    : min_area_ratio_(min_area_percent / 100.0) {}

namespace {

// Angle (degrees) at point `far` in the triangle (a, far, b).
double angle_deg(const cv::Point& a, const cv::Point& b, const cv::Point& far) {
    double ax = a.x - far.x, ay = a.y - far.y;
    double bx = b.x - far.x, by = b.y - far.y;
    double dot = ax * bx + ay * by;
    double magA = std::sqrt(ax * ax + ay * ay);
    double magB = std::sqrt(bx * bx + by * by);
    if (magA < 1e-6 || magB < 1e-6) return 180.0;
    double c = std::clamp(dot / (magA * magB), -1.0, 1.0);
    return std::acos(c) * 180.0 / CV_PI;
}

}  // namespace

bool HandDetector::detect(const cv::Mat& frame, float& openness,
                          float& confidence, cv::Mat* overlay) {
    if (frame.empty()) return false;

    // ── 1. Skin-colour mask in YCrCb (robust to brightness) ──
    cv::Mat ycrcb;
    cv::cvtColor(frame, ycrcb, cv::COLOR_BGR2YCrCb);

    cv::Mat mask;
    cv::inRange(ycrcb, cv::Scalar(0, 133, 77), cv::Scalar(255, 173, 127), mask);

    // Clean up noise
    cv::Mat kernel = cv::getStructuringElement(cv::MORPH_ELLIPSE, {7, 7});
    cv::morphologyEx(mask, mask, cv::MORPH_OPEN, kernel);
    cv::morphologyEx(mask, mask, cv::MORPH_CLOSE, kernel);
    cv::GaussianBlur(mask, mask, {5, 5}, 0);
    cv::threshold(mask, mask, 60, 255, cv::THRESH_BINARY);

    // ── 2. Largest skin contour = the hand ──
    std::vector<std::vector<cv::Point>> contours;
    cv::findContours(mask, contours, cv::RETR_EXTERNAL, cv::CHAIN_APPROX_SIMPLE);
    if (contours.empty()) return false;

    int best = -1;
    double best_area = 0.0;
    for (int i = 0; i < static_cast<int>(contours.size()); ++i) {
        double a = cv::contourArea(contours[i]);
        if (a > best_area) { best_area = a; best = i; }
    }

    double frame_area = static_cast<double>(frame.cols) * frame.rows;
    // Reject if too small (hand far/absent) or almost full frame (bad lighting)
    if (best < 0 || best_area < min_area_ratio_ * frame_area
        || best_area > 0.90 * frame_area) {
        return false;
    }

    const auto& hand = contours[best];

    // ── 3. Convex hull + convexity defects → finger valleys ──
    std::vector<cv::Point> hull_pts;
    std::vector<int> hull_idx;
    cv::convexHull(hand, hull_pts);
    cv::convexHull(hand, hull_idx, false, false);

    double hull_area = cv::contourArea(hull_pts);
    double solidity = hull_area > 1.0 ? best_area / hull_area : 1.0;

    int finger_gaps = 0;
    std::vector<cv::Point> defect_pts;
    if (hull_idx.size() > 3) {
        std::vector<cv::Vec4i> defects;
        cv::convexityDefects(hand, hull_idx, defects);

        double diag = std::sqrt(frame_area);
        double min_depth = 0.05 * diag;  // ignore shallow bumps

        for (const auto& d : defects) {
            const cv::Point& start = hand[d[0]];
            const cv::Point& end = hand[d[1]];
            const cv::Point& far = hand[d[2]];
            double depth = d[3] / 256.0;

            // A finger valley is deep AND has an acute angle at its bottom.
            if (depth > min_depth && angle_deg(start, end, far) < 95.0) {
                ++finger_gaps;
                defect_pts.push_back(far);
            }
        }
    }

    // ── 4. Openness: finger valleys are the strongest signal, refined by
    //         solidity (a fist is a compact, high-solidity blob). ──
    float from_fingers = std::min(1.0f, finger_gaps / 4.0f);
    float from_solidity = static_cast<float>(std::clamp((0.90 - solidity) / 0.30, 0.0, 1.0));
    openness = 0.70f * from_fingers + 0.30f * from_solidity;
    openness = std::clamp(openness, 0.0f, 1.0f);

    confidence = static_cast<float>(std::min(1.0, best_area / (0.25 * frame_area)));

    // ── 5. Optional debug overlay ──
    if (overlay) {
        cv::drawContours(*overlay, contours, best, {0, 200, 0}, 2);
        std::vector<std::vector<cv::Point>> hulls{hull_pts};
        cv::drawContours(*overlay, hulls, 0, {0, 160, 255}, 2);
        for (const auto& p : defect_pts)
            cv::circle(*overlay, p, 6, {0, 0, 255}, -1);
    }

    return true;
}
