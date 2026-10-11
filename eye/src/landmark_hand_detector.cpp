#include "landmark_hand_detector.hpp"

#include <opencv2/imgproc.hpp>

#include <algorithm>
#include <cmath>
#include <limits>

namespace {
constexpr int kPalmInput = 192;
constexpr int kHandInput = 224;
constexpr float kPalmThreshold = 0.55f;
constexpr float kHandThreshold = 0.65f;
constexpr int kReleaseAfterMisses = 4;

float sigmoid(float value) {
    if (value >= 0.0f) return 1.0f / (1.0f + std::exp(-value));
    const float e = std::exp(value);
    return e / (1.0f + e);
}

float clamp01(float value) {
    return std::max(0.0f, std::min(1.0f, value));
}

float joint_angle(const cv::Point3f& a, const cv::Point3f& joint,
                  const cv::Point3f& b) {
    const cv::Point3f u = a - joint;
    const cv::Point3f v = b - joint;
    const float denom = std::sqrt(u.dot(u) * v.dot(v));
    if (denom < 1e-6f) return 0.0f;
    const float cosine = std::max(-1.0f, std::min(1.0f, u.dot(v) / denom));
    return std::acos(cosine) * 180.0f / static_cast<float>(CV_PI);
}

float planar_distance(const cv::Point3f& a, const cv::Point3f& b) {
    return std::hypot(a.x - b.x, a.y - b.y);
}

struct CropResult {
    cv::Mat image;
    cv::Rect2f box;
    cv::Point2f bias;
};

CropResult crop_and_pad(const cv::Mat& image, cv::Rect2f box,
                        bool for_rotation) {
    const float shift_y = for_rotation ? 0.0f : -0.4f;
    const float enlarge = for_rotation ? 4.0f : 3.0f;
    const cv::Point2f size(box.width, box.height);
    cv::Point2f center(box.x + box.width * 0.5f,
                       box.y + box.height * 0.5f);
    center.y += shift_y * size.y;

    const cv::Point2f half(size.x * enlarge * 0.5f,
                           size.y * enlarge * 0.5f);
    int x1 = std::clamp(static_cast<int>(center.x - half.x), 0, image.cols);
    int y1 = std::clamp(static_cast<int>(center.y - half.y), 0, image.rows);
    int x2 = std::clamp(static_cast<int>(center.x + half.x), 0, image.cols);
    int y2 = std::clamp(static_cast<int>(center.y + half.y), 0, image.rows);
    if (x2 <= x1 || y2 <= y1) return {};

    cv::Mat crop = image(cv::Rect(x1, y1, x2 - x1, y2 - y1)).clone();
    int side = for_rotation
        ? static_cast<int>(std::sqrt(static_cast<double>(crop.rows * crop.rows +
                                                         crop.cols * crop.cols)))
        : std::max(crop.rows, crop.cols);
    side = std::max(side, std::max(crop.rows, crop.cols));
    const int pad_h = side - crop.rows;
    const int pad_w = side - crop.cols;
    const int left = pad_w / 2;
    const int top = pad_h / 2;
    cv::copyMakeBorder(crop, crop, top, pad_h - top, left, pad_w - left,
                       cv::BORDER_CONSTANT, cv::Scalar(0, 0, 0));

    return {crop, cv::Rect2f(static_cast<float>(x1), static_cast<float>(y1),
                             static_cast<float>(x2 - x1), static_cast<float>(y2 - y1)),
            cv::Point2f(static_cast<float>(x1 - left),
                        static_cast<float>(y1 - top))};
}

cv::Point2f transform_point(const cv::Mat& matrix, const cv::Point2f& point) {
    return {
        static_cast<float>(matrix.at<double>(0, 0) * point.x +
                           matrix.at<double>(0, 1) * point.y +
                           matrix.at<double>(0, 2)),
        static_cast<float>(matrix.at<double>(1, 0) * point.x +
                           matrix.at<double>(1, 1) * point.y +
                           matrix.at<double>(1, 2))
    };
}

cv::Rect2f bounds_of(const std::array<cv::Point2f, 7>& points) {
    float min_x = std::numeric_limits<float>::max();
    float min_y = std::numeric_limits<float>::max();
    float max_x = std::numeric_limits<float>::lowest();
    float max_y = std::numeric_limits<float>::lowest();
    for (const auto& point : points) {
        min_x = std::min(min_x, point.x);
        min_y = std::min(min_y, point.y);
        max_x = std::max(max_x, point.x);
        max_y = std::max(max_y, point.y);
    }
    return {min_x, min_y, max_x - min_x, max_y - min_y};
}
} // namespace

LandmarkHandDetector::LandmarkHandDetector(const std::string& palm_onnx,
                                           const std::string& handpose_onnx) {
    if (palm_onnx.empty() || handpose_onnx.empty()) {
        error_ = "model file missing";
        return;
    }

    try {
        std::vector<uchar> palm_bytes(palm_onnx.begin(), palm_onnx.end());
        std::vector<uchar> hand_bytes(handpose_onnx.begin(), handpose_onnx.end());
        palm_net_ = cv::dnn::readNetFromONNX(palm_bytes);
        handpose_net_ = cv::dnn::readNetFromONNX(hand_bytes);
        palm_net_.setPreferableBackend(cv::dnn::DNN_BACKEND_OPENCV);
        palm_net_.setPreferableTarget(cv::dnn::DNN_TARGET_CPU);
        handpose_net_.setPreferableBackend(cv::dnn::DNN_BACKEND_OPENCV);
        handpose_net_.setPreferableTarget(cv::dnn::DNN_TARGET_CPU);
        anchors_ = make_anchors();
        ready_ = !palm_net_.empty() && !handpose_net_.empty() && anchors_.size() == 2016;
        if (!ready_) error_ = "model initialization failed";
    } catch (const cv::Exception& ex) {
        error_ = ex.what();
    }
}

std::vector<cv::Point2f> LandmarkHandDetector::make_anchors() {
    std::vector<cv::Point2f> anchors;
    anchors.reserve(2016);
    for (int y = 0; y < 24; ++y)
        for (int x = 0; x < 24; ++x)
            for (int repeat = 0; repeat < 2; ++repeat)
                anchors.emplace_back((x + 0.5f) / 24.0f, (y + 0.5f) / 24.0f);
    for (int y = 0; y < 12; ++y)
        for (int x = 0; x < 12; ++x)
            for (int repeat = 0; repeat < 6; ++repeat)
                anchors.emplace_back((x + 0.5f) / 12.0f, (y + 0.5f) / 12.0f);
    return anchors;
}

bool LandmarkHandDetector::detect_palm(const cv::Mat& frame, Palm& palm) {
    const float ratio = std::min(static_cast<float>(kPalmInput) / frame.cols,
                                 static_cast<float>(kPalmInput) / frame.rows);
    const cv::Size resized_size(std::max(1, static_cast<int>(frame.cols * ratio)),
                                std::max(1, static_cast<int>(frame.rows * ratio)));
    cv::Mat prepared;
    cv::resize(frame, prepared, resized_size);
    const int pad_w = kPalmInput - prepared.cols;
    const int pad_h = kPalmInput - prepared.rows;
    const int left = pad_w / 2;
    const int top = pad_h / 2;
    cv::copyMakeBorder(prepared, prepared, top, pad_h - top, left, pad_w - left,
                       cv::BORDER_CONSTANT, cv::Scalar(0, 0, 0));

    cv::dnn::Image2BlobParams params;
    params.datalayout = cv::dnn::DNN_LAYOUT_NHWC;
    params.ddepth = CV_32F;
    params.scalefactor = cv::Scalar::all(1.0 / 255.0);
    params.size = cv::Size(kPalmInput, kPalmInput);
    params.swapRB = true;
    params.paddingmode = cv::dnn::DNN_PMODE_NULL;
    palm_net_.setInput(cv::dnn::blobFromImageWithParams(prepared, params));

    std::vector<cv::Mat> outputs;
    palm_net_.forward(outputs, palm_net_.getUnconnectedOutLayersNames());
    if (outputs.size() < 2) return false;
    cv::Mat boxes = outputs[0].reshape(1, static_cast<int>(outputs[0].total() / 18));
    cv::Mat scores = outputs[1].reshape(1, static_cast<int>(outputs[1].total()));
    if (boxes.rows != static_cast<int>(anchors_.size()) || scores.rows != boxes.rows)
        return false;

    int best = -1;
    float best_score = kPalmThreshold;
    for (int i = 0; i < scores.rows; ++i) {
        const float score = sigmoid(scores.at<float>(i, 0));
        if (score > best_score) {
            best = i;
            best_score = score;
        }
    }
    if (best < 0) return false;

    const float scale = static_cast<float>(std::max(frame.cols, frame.rows));
    const cv::Point2f pad_bias(left / ratio, top / ratio);
    const float* values = boxes.ptr<float>(best);
    const cv::Point2f anchor = anchors_[best];
    const cv::Point2f center((values[0] / kPalmInput + anchor.x) * scale - pad_bias.x,
                             (values[1] / kPalmInput + anchor.y) * scale - pad_bias.y);
    const cv::Point2f size(values[2] / kPalmInput * scale,
                           values[3] / kPalmInput * scale);
    palm.box = {center.x - size.x * 0.5f, center.y - size.y * 0.5f,
                size.x, size.y};
    for (int i = 0; i < 7; ++i) {
        palm.landmarks[i] = {
            (values[4 + i * 2] / kPalmInput + anchor.x) * scale - pad_bias.x,
            (values[5 + i * 2] / kPalmInput + anchor.y) * scale - pad_bias.y
        };
    }
    palm.confidence = best_score;
    return palm.box.width > 2.0f && palm.box.height > 2.0f;
}

bool LandmarkHandDetector::estimate_hand(const cv::Mat& frame, const Palm& palm,
                                         HandResult& hand) {
    CropResult initial = crop_and_pad(frame, palm.box, true);
    if (initial.image.empty()) return false;
    cv::cvtColor(initial.image, initial.image, cv::COLOR_BGR2RGB);

    cv::Rect2f local_box(palm.box.x - initial.bias.x,
                         palm.box.y - initial.bias.y,
                         palm.box.width, palm.box.height);
    std::array<cv::Point2f, 7> local_landmarks{};
    for (int i = 0; i < 7; ++i)
        local_landmarks[i] = palm.landmarks[i] - initial.bias;

    const cv::Point2f p1 = local_landmarks[0];
    const cv::Point2f p2 = local_landmarks[2];
    double radians = CV_PI / 2.0 - std::atan2(-(p2.y - p1.y), p2.x - p1.x);
    radians -= 2.0 * CV_PI * std::floor((radians + CV_PI) / (2.0 * CV_PI));
    const double angle = radians * 180.0 / CV_PI;
    const cv::Point2f palm_center(local_box.x + local_box.width * 0.5f,
                                  local_box.y + local_box.height * 0.5f);
    const cv::Mat rotation = cv::getRotationMatrix2D(palm_center, angle, 1.0);

    cv::Mat rotated;
    cv::warpAffine(initial.image, rotated, rotation, initial.image.size());
    std::array<cv::Point2f, 7> rotated_landmarks{};
    for (int i = 0; i < 7; ++i)
        rotated_landmarks[i] = transform_point(rotation, local_landmarks[i]);
    const cv::Rect2f rotated_palm_bounds = bounds_of(rotated_landmarks);
    CropResult hand_crop = crop_and_pad(rotated, rotated_palm_bounds, false);
    if (hand_crop.image.empty()) return false;

    cv::dnn::Image2BlobParams params;
    params.datalayout = cv::dnn::DNN_LAYOUT_NHWC;
    params.ddepth = CV_32F;
    params.scalefactor = cv::Scalar::all(1.0 / 255.0);
    params.size = cv::Size(kHandInput, kHandInput);
    params.swapRB = false;
    params.paddingmode = cv::dnn::DNN_PMODE_NULL;
    handpose_net_.setInput(cv::dnn::blobFromImageWithParams(hand_crop.image, params));

    std::vector<cv::Mat> outputs;
    handpose_net_.forward(outputs, handpose_net_.getUnconnectedOutLayersNames());
    if (outputs.size() < 4 || outputs[0].total() < 63 || outputs[1].empty() ||
        outputs[2].empty()) return false;
    hand.confidence = outputs[1].ptr<float>()[0];
    hand.handedness = outputs[2].ptr<float>()[0];
    if (hand.confidence < kHandThreshold) return false;

    const float scale = std::max(hand_crop.box.width, hand_crop.box.height) /
                        static_cast<float>(kHandInput);
    const cv::Point2f crop_center(hand_crop.box.x + hand_crop.box.width * 0.5f,
                                  hand_crop.box.y + hand_crop.box.height * 0.5f);
    const cv::Mat around_origin = cv::getRotationMatrix2D(cv::Point2f(0, 0),
                                                          angle, 1.0);

    const double r00 = rotation.at<double>(0, 0);
    const double r01 = rotation.at<double>(0, 1);
    const double r10 = rotation.at<double>(1, 0);
    const double r11 = rotation.at<double>(1, 1);
    const double tx = rotation.at<double>(0, 2);
    const double ty = rotation.at<double>(1, 2);
    const cv::Point2f original_center(
        static_cast<float>(r00 * crop_center.x + r10 * crop_center.y - r00 * tx - r10 * ty),
        static_cast<float>(r01 * crop_center.x + r11 * crop_center.y - r01 * tx - r11 * ty));

    const float* raw = outputs[0].ptr<float>();
    float min_x = std::numeric_limits<float>::max();
    float min_y = std::numeric_limits<float>::max();
    float max_x = std::numeric_limits<float>::lowest();
    float max_y = std::numeric_limits<float>::lowest();
    for (int i = 0; i < 21; ++i) {
        const float x = (raw[i * 3] - kHandInput * 0.5f) * scale;
        const float y = (raw[i * 3 + 1] - kHandInput * 0.5f) * scale;
        const float z = raw[i * 3 + 2] * scale;
        // Match the row-vector transform used by the OpenCV Zoo reference.
        const float rx = static_cast<float>(x * around_origin.at<double>(0, 0) +
                                             y * around_origin.at<double>(1, 0));
        const float ry = static_cast<float>(x * around_origin.at<double>(0, 1) +
                                             y * around_origin.at<double>(1, 1));
        const float image_x = rx + original_center.x + initial.bias.x;
        const float image_y = ry + original_center.y + initial.bias.y;
        hand.landmarks[i] = {image_x, image_y, z};
        min_x = std::min(min_x, image_x);
        min_y = std::min(min_y, image_y);
        max_x = std::max(max_x, image_x);
        max_y = std::max(max_y, image_y);
    }
    hand.box = {min_x, min_y, max_x - min_x, max_y - min_y};
    return hand.box.width > 2.0f && hand.box.height > 2.0f;
}

LandmarkHandDetector::Palm LandmarkHandDetector::palm_from_hand(
    const HandResult& hand) {
    static constexpr int ids[] = {0, 5, 9, 13, 17, 1, 2};
    Palm palm;
    float min_x = std::numeric_limits<float>::max();
    float min_y = std::numeric_limits<float>::max();
    float max_x = std::numeric_limits<float>::lowest();
    float max_y = std::numeric_limits<float>::lowest();
    for (int i = 0; i < 7; ++i) {
        const auto& point = hand.landmarks[ids[i]];
        palm.landmarks[i] = {point.x, point.y};
        min_x = std::min(min_x, point.x);
        min_y = std::min(min_y, point.y);
        max_x = std::max(max_x, point.x);
        max_y = std::max(max_y, point.y);
    }
    palm.box = {min_x, min_y, max_x - min_x, max_y - min_y};
    palm.confidence = hand.confidence;
    return palm;
}

float LandmarkHandDetector::measure_openness(
    const std::array<cv::Point3f, 21>& landmarks, int& extended_fingers) {
    const int fingers[4][4] = {{5, 6, 7, 8}, {9, 10, 11, 12},
                               {13, 14, 15, 16}, {17, 18, 19, 20}};
    float total = 0.0f;
    extended_fingers = 0;
    for (const auto& finger : fingers) {
        const float pip = joint_angle(landmarks[finger[0]], landmarks[finger[1]],
                                      landmarks[finger[2]]);
        const float dip = joint_angle(landmarks[finger[1]], landmarks[finger[2]],
                                      landmarks[finger[3]]);
        const float score = clamp01((std::min(pip, dip) - 80.0f) / 75.0f);
        total += score;
        if (score >= 0.65f) extended_fingers++;
    }

    // A straight thumb can still be folded across the front of a fist, so
    // joint angles alone over-report it. Its reach is measured from the
    // pinky MCP in palm-width units: the index->pinky axis already encodes
    // which side the thumb sits on, so this works for either hand and for
    // palm or back facing the camera without trusting the model's
    // handedness (inverted anyway because the camera frame is mirrored).
    const float thumb_mcp = joint_angle(landmarks[1], landmarks[2], landmarks[3]);
    const float thumb_ip = joint_angle(landmarks[2], landmarks[3], landmarks[4]);
    float thumb_score = clamp01((std::min(thumb_mcp, thumb_ip) - 75.0f) / 75.0f);
    const float palm_width = planar_distance(landmarks[5], landmarks[17]);
    const float palm_length = planar_distance(landmarks[0], landmarks[9]);
    // Edge-on hands collapse the palm width; reach is meaningless there.
    if (palm_width > 0.25f * palm_length && palm_width > 1e-6f) {
        const float reach = planar_distance(landmarks[4], landmarks[17]) / palm_width;
        thumb_score = std::min(thumb_score, clamp01((reach - 0.9f) / 0.4f));
    }
    total += thumb_score;
    if (thumb_score >= 0.65f) extended_fingers++;
    return clamp01(total / 5.0f);
}

void LandmarkHandDetector::mark_missed() {
    lost_frames_++;
    if (locked_ && lost_frames_ >= kReleaseAfterMisses) {
        locked_ = false;
        event_ = "[Eye] Landmark hand lock released";
    }
}

bool LandmarkHandDetector::track(const cv::Mat& frame, HandObservation& obs,
                                 cv::Mat* overlay) {
    if (!ready_ || frame.empty()) return false;
    try {
        HandResult hand;
        bool found = false;

        // Once a hand is known, the previous frame's palm landmarks define a
        // generous rotated ROI. HandPose is much cheaper than PalmDet, so this
        // path removes the full-frame detector from steady-state tracking.
        if (have_tracked_palm_)
            found = estimate_hand(frame, tracked_palm_, hand);

        // First frame, fast motion or an occlusion: immediately reacquire on
        // the same frame instead of waiting for a periodic detector pass.
        if (!found) {
            Palm detected;
            if (detect_palm(frame, detected))
                found = estimate_hand(frame, detected, hand);
        }

        if (!found) {
            have_tracked_palm_ = false;
            mark_missed();
            return false;
        }

        tracked_palm_ = palm_from_hand(hand);
        have_tracked_palm_ = true;
        lost_frames_ = 0;
        if (!locked_) {
            locked_ = true;
            event_ = "[Eye] Landmark hand locked";
        }
        int fingers = 0;
        const float openness = measure_openness(hand.landmarks, fingers);
        obs = {};
        obs.openness = openness;
        obs.confidence = hand.confidence;
        obs.cx = (hand.box.x + hand.box.width * 0.5f) / frame.cols;
        obs.cy = (hand.box.y + hand.box.height * 0.5f) / frame.rows;
        obs.fingers = fingers;
        obs.has_landmarks = true;
        obs.landmarks = hand.landmarks;
        obs.handedness = hand.handedness;
        if (overlay) draw_hand(*overlay, hand, openness);
        return true;
    } catch (const cv::Exception& ex) {
        error_ = ex.what();
        event_ = "[Eye] Landmark inference error; tracking frame skipped";
        mark_missed();
        return false;
    }
}

void LandmarkHandDetector::draw_hand(cv::Mat& image, const HandResult& hand,
                                     float openness) {
    static constexpr int edges[][2] = {
        {0,1},{1,2},{2,3},{3,4}, {0,5},{5,6},{6,7},{7,8},
        {5,9},{9,10},{10,11},{11,12}, {9,13},{13,14},{14,15},{15,16},
        {13,17},{17,18},{18,19},{19,20},{17,0}
    };
    for (const auto& edge : edges) {
        const auto& a = hand.landmarks[edge[0]];
        const auto& b = hand.landmarks[edge[1]];
        cv::line(image, cv::Point(cvRound(a.x), cvRound(a.y)),
                 cv::Point(cvRound(b.x), cvRound(b.y)), cv::Scalar(255, 180, 0), 2,
                 cv::LINE_AA);
    }
    for (const auto& point : hand.landmarks)
        cv::circle(image, cv::Point(cvRound(point.x), cvRound(point.y)), 3,
                   cv::Scalar(0, 255, 255), cv::FILLED, cv::LINE_AA);
    cv::putText(image, cv::format("landmark %.2f conf %.2f", openness,
                                  hand.confidence),
                cv::Point(std::max(0, cvRound(hand.box.x)),
                          std::max(18, cvRound(hand.box.y) - 6)),
                cv::FONT_HERSHEY_SIMPLEX, 0.5, cv::Scalar(0, 255, 255), 1,
                cv::LINE_AA);
}

void LandmarkHandDetector::reset() {
    locked_ = false;
    lost_frames_ = 0;
    have_tracked_palm_ = false;
    event_.clear();
}

std::string LandmarkHandDetector::take_event() {
    std::string value = event_;
    event_.clear();
    return value;
}

