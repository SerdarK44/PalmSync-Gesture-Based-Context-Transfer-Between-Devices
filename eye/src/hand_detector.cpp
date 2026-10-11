#include "hand_detector.hpp"

#include <opencv2/imgproc.hpp>

#include <algorithm>
#include <climits>
#include <cmath>
#include <cstdio>
#include <string>
#include <vector>

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

double dist(cv::Point2f a, cv::Point2f b) {
    double dx = a.x - b.x, dy = a.y - b.y;
    return std::sqrt(dx * dx + dy * dy);
}

// ── Tuning ──
constexpr int    kFaceEvery      = 2;     // run face detection every N frames
constexpr int    kFaceMemory     = 60;    // keep masking a face ~2 s after the cascades last saw it...
constexpr int    kFaceHoldMax    = 300;   // ...or up to ~10 s while its skin is still there
constexpr double kFaceHoldSkin   = 0.35;  // fraction of the face box that must still be skin
constexpr int    kLostFrames     = 18;    // ~0.6 s at 30 fps without the hand -> unlock
constexpr int    kConfirmFrames  = 2;     // ~65 ms at 30 fps: near-instant credible-hand lock
constexpr int    kOpenFingers    = 3;     // ring crossings needed for an open hand
constexpr int    kOpenGaps       = 2;     // ...and convexity valleys that agree
constexpr double kOpenMaxSolidity = 0.85; // ...and real space between the fingers
constexpr double kMaxContactOpen = 0.08;  // an open hand may touch the face mask with <= 8% of its outline
constexpr double kGate           = 0.22;  // max jump between frames (fraction of frame diagonal)
constexpr double kMinAreaRatio   = 0.30;  // tracked blob may shrink to 30% (open -> fist)...
constexpr double kMaxAreaRatio   = 1.8;   // ...but a hand never grows 1.8x in one frame (= merged)
constexpr double kMergedPalm     = 1.5;   // palm radius grew this much -> merged with face/body
constexpr double kRecoveryGate   = 0.32;  // relaxed same-hand gate while fingers fold into a fist
constexpr double kRecoveryMinArea = 0.18; // tolerate the contour shrinking sharply while closing
constexpr double kRecoveryMaxArea = 2.8;  // but never jump to a much larger face/body blob
constexpr double kRecoveryMaxContact = 0.30; // a partially masked hand may touch the face halo

}  // namespace

HandDetector::HandDetector(int min_area_percent)
    : min_area_ratio_(min_area_percent / 100.0) {}

static bool load_cascade(cv::CascadeClassifier& c, const std::string& xml) {
    if (xml.empty()) return false;
    try {
        cv::FileStorage fs(xml, cv::FileStorage::READ | cv::FileStorage::MEMORY);
        return fs.isOpened() && c.read(fs.getFirstTopLevelNode()) && !c.empty();
    } catch (const cv::Exception&) {
        return false;
    }
}

bool HandDetector::load_face_model(const std::string& xml) {
    face_ok_ = load_cascade(face_, xml);
    return face_ok_;
}

bool HandDetector::load_profile_model(const std::string& xml) {
    profile_ok_ = load_cascade(profile_, xml);
    return profile_ok_;
}

void HandDetector::reset() {
    locked_ = false;
    lost_frames_ = 0;
    pending_frames_ = 0;
}

std::string HandDetector::take_event() {
    std::string e;
    e.swap(event_);
    return e;
}

void HandDetector::set_faces_for_test(const std::vector<cv::Rect>& faces) {
    faces_ = faces;
    manual_faces_ = true;
    face_ok_ = true;
    frames_since_face_ = 0;
}

// ── Skin & blobs ────────────────────────────────────────────

cv::Mat HandDetector::skin_mask(const cv::Mat& frame) const {
    // Skin colour in YCrCb (robust to brightness)
    cv::Mat ycrcb, mask;
    cv::cvtColor(frame, ycrcb, cv::COLOR_BGR2YCrCb);
    cv::inRange(ycrcb, cv::Scalar(0, 133, 77), cv::Scalar(255, 173, 127), mask);

    // Clean up noise
    cv::Mat kernel = cv::getStructuringElement(cv::MORPH_ELLIPSE, {7, 7});
    cv::morphologyEx(mask, mask, cv::MORPH_OPEN, kernel);
    cv::morphologyEx(mask, mask, cv::MORPH_CLOSE, kernel);
    cv::GaussianBlur(mask, mask, {5, 5}, 0);
    cv::threshold(mask, mask, 60, 255, cv::THRESH_BINARY);
    return mask;
}

std::vector<HandDetector::Blob> HandDetector::blobs(const cv::Mat& mask, double frame_area) const {
    std::vector<std::vector<cv::Point>> contours;
    cv::findContours(mask, contours, cv::RETR_EXTERNAL, cv::CHAIN_APPROX_SIMPLE);

    const double diag = std::sqrt(frame_area);
    std::vector<Blob> out;
    for (auto& c : contours) {
        double a = cv::contourArea(c);
        // too small (noise / far away) or almost the whole frame (bad lighting)
        if (a < min_area_ratio_ * frame_area || a > 0.90 * frame_area) continue;
        Blob b;
        b.contour = std::move(c);
        b.area = a;
        b.box = cv::boundingRect(b.contour);
        cv::Moments m = cv::moments(b.contour);
        b.center = m.m00 > 1e-6 ? cv::Point2f(float(m.m10 / m.m00), float(m.m01 / m.m00))
                                : cv::Point2f(b.box.x + b.box.width / 2.0f, b.box.y + b.box.height / 2.0f);
        measure(b, diag);
        out.push_back(std::move(b));
    }
    return out;
}

// Palm-circle finger count: the largest inscribed circle is the palm; walk a
// ring around it and count the strips of skin that cross it with a FINGER's
// width. Wider strips (wrist/arm, a face band) don't count; a face or a fist
// has nothing out there at all.
void HandDetector::palm_and_fingers(Blob& b) {
    const int pad = 2;
    cv::Mat m = cv::Mat::zeros(b.box.height + 2 * pad, b.box.width + 2 * pad, CV_8U);
    std::vector<std::vector<cv::Point>> cs{b.contour};
    cv::drawContours(m, cs, 0, cv::Scalar(255), cv::FILLED, cv::LINE_8, cv::noArray(), INT_MAX,
                     cv::Point(pad - b.box.x, pad - b.box.y));

    cv::Mat d;
    cv::distanceTransform(m, d, cv::DIST_L2, 3);
    double maxv = 0;
    cv::Point maxp;
    cv::minMaxLoc(d, nullptr, &maxv, nullptr, &maxp);
    b.palm_r = static_cast<float>(maxv);
    b.palm = cv::Point2f(float(maxp.x - pad + b.box.x), float(maxp.y - pad + b.box.y));
    b.fingers = 0;
    if (maxv < 4) return;

    const int N = 240;
    for (double k : {1.45, 1.7, 1.95, 2.2}) {
        const double r = k * maxv;
        std::vector<unsigned char> v(N);
        int ones = 0;
        for (int i = 0; i < N; ++i) {
            double a = 2 * CV_PI * i / N;
            int x = int(std::lround(maxp.x + r * std::cos(a)));
            int y = int(std::lround(maxp.y + r * std::sin(a)));
            v[i] = (x >= 0 && y >= 0 && x < m.cols && y < m.rows && m.at<unsigned char>(y, x)) ? 1 : 0;
            ones += v[i];
        }
        if (ones == 0 || ones == N) continue;

        // Spread fingers have real empty space between them. Thin dark cuts
        // (hair strands over a forehead, glasses, shadows) are not gaps:
        // close any gap narrower than 0.2·palm radius before counting.
        {
            const double min_gap = 0.2 * maxv;
            int z = 0;
            while (v[z] == 0 && z < N) ++z;   // start inside skin
            for (int j = 0; j < N; ++j) {
                int i = (z + j) % N;
                if (v[i] == 0 && v[(i + N - 1) % N] == 1) {
                    int len = 0;
                    while (v[(i + len) % N] == 0 && len < N) ++len;
                    double chord = 2 * r * std::sin(len * CV_PI / N);
                    if (chord < min_gap)
                        for (int k2 = 0; k2 < len; ++k2) v[(i + k2) % N] = 1;
                }
            }
            ones = 0;
            for (auto x : v) ones += x;
            if (ones == N) continue;
        }

        int start = 0;
        while (v[start] == 1) ++start;   // begin on a gap so runs don't wrap
        int count = 0;
        std::vector<cv::Point> pts;
        for (int j = 1; j <= N; ++j) {
            int i = (start + j) % N;
            if (v[i] == 1 && v[(i + N - 1) % N] == 0) {
                int len = 0;
                while (v[(i + len) % N] == 1 && len < N) ++len;
                double dtheta = len * 2 * CV_PI / N;
                double chord = 2 * r * std::sin(dtheta / 2);
                if (chord >= 0.18 * maxv && chord <= 0.9 * maxv) {
                    ++count;
                    double mid = 2 * CV_PI * (i + len / 2.0) / N;
                    pts.emplace_back(int(maxp.x - pad + b.box.x + r * std::cos(mid)),
                                     int(maxp.y - pad + b.box.y + r * std::sin(mid)));
                }
            }
        }
        if (count > b.fingers) {
            b.fingers = count;
            b.ring_r = static_cast<float>(r);
            b.finger_pts = std::move(pts);
        }
    }
}

// Convexity valleys + palm-circle fingers -> openness 0..1.
void HandDetector::measure(Blob& b, double diag) {
    std::vector<int> hull_idx;
    cv::convexHull(b.contour, b.hull);
    cv::convexHull(b.contour, hull_idx, false, false);

    double hull_area = cv::contourArea(b.hull);
    b.solidity = hull_area > 1.0 ? b.area / hull_area : 1.0;

    b.gaps = 0;
    if (hull_idx.size() > 3) {
        std::vector<cv::Vec4i> defects;
        try {
            cv::convexityDefects(b.contour, hull_idx, defects);
        } catch (const cv::Exception&) {
            defects.clear();  // self-intersecting contour: no reliable valleys
        }
        double min_depth = 0.05 * diag;  // ignore shallow bumps
        for (const auto& d : defects) {
            const cv::Point& start = b.contour[d[0]];
            const cv::Point& end = b.contour[d[1]];
            const cv::Point& far = b.contour[d[2]];
            double depth = d[3] / 256.0;
            // A finger valley is deep AND has an acute angle at its bottom.
            if (depth > min_depth && angle_deg(start, end, far) < 90.0) {
                ++b.gaps;
                b.valleys.push_back(far);
            }
        }
    }

    palm_and_fingers(b);

    float from_fingers = std::min(1.0f, b.fingers / 4.0f);
    float from_gaps = std::min(1.0f, b.gaps / 4.0f);
    float from_solidity = static_cast<float>(std::clamp((0.90 - b.solidity) / 0.30, 0.0, 1.0));
    // Both finger signals count equally, so one of them flickering (a finger
    // cut by the image edge, two fingers touching) doesn't flip the pose.
    b.openness = std::clamp(0.42f * from_fingers + 0.43f * from_gaps + 0.15f * from_solidity, 0.0f, 1.0f);
}

bool HandDetector::is_open_hand(const Blob& b) const {
    // Spread fingers leave real empty space inside the convex hull: an open
    // hand covers ~55–75 % of its hull; a face with thin notches ~90 %+.
    return b.fingers >= kOpenFingers && b.gaps >= kOpenGaps && b.solidity < kOpenMaxSolidity;
}

// A fist can only be told apart from a face when we know where the face is.
bool HandDetector::is_fist(const Blob& b, double frame_area) const {
    if (faces_.empty() || b.fingers > 1) return false;
    double aspect = b.box.width / std::max(1.0, double(b.box.height));
    if (b.area < 0.01 * frame_area || b.area > 0.20 * frame_area) return false;
    if (b.solidity < 0.80 || aspect < 0.5 || aspect > 2.0) return false;
    // A fist is clearly smaller than the face it's next to.
    int face_w = 0;
    for (const auto& f : faces_) face_w = std::max(face_w, f.width);
    return b.palm_r <= 0.5 * face_w;
}

// How much of the blob's outline runs along the face mask. Leftovers of the
// face (cheek, ear, jaw strips) are cut out BY the mask, so a large part of
// their outline lies on it; a hand beside the face at most touches it with
// a fingertip.
double HandDetector::mask_contact(const Blob& b) const {
    if (face_halo_.empty() || b.contour.size() < 2) return 0.0;
    auto on = [&](const cv::Point& p) {
        return p.x >= 0 && p.y >= 0 && p.x < face_halo_.cols && p.y < face_halo_.rows &&
               face_halo_.at<unsigned char>(p) != 0;
    };
    double total = 0, along = 0;
    const size_t n = b.contour.size();
    for (size_t i = 0; i < n; ++i) {
        const cv::Point& a = b.contour[i];
        const cv::Point& c = b.contour[(i + 1) % n];
        double len = std::hypot(double(c.x - a.x), double(c.y - a.y));
        total += len;
        if (on(a) && on(c) && on((a + c) / 2)) along += len;
    }
    return total > 0 ? along / total : 0.0;
}

// ── Faces ───────────────────────────────────────────────────

void HandDetector::update_faces(const cv::Mat& frame, const cv::Mat& skin) {
    if (manual_faces_) return;
    ++frames_since_face_;

    // The cascade misses faces now and then — especially with a hand right
    // next to them, exactly when masking matters most. If a face box is
    // dropped, face + hand skin merge into one blob and the tag slides onto
    // the face. So keep the box while it is still mostly skin.
    auto drop_stale = [&] {
        if (frames_since_face_ <= kFaceMemory || faces_.empty()) return;
        bool still_there = frames_face_held_ < kFaceHoldMax;
        for (const auto& f : faces_) {
            cv::Rect r = f & cv::Rect(0, 0, skin.cols, skin.rows);
            if (r.area() == 0 || cv::countNonZero(skin(r)) < kFaceHoldSkin * r.area()) still_there = false;
        }
        if (still_there) ++frames_face_held_;
        else { faces_.clear(); frames_face_held_ = 0; }
    };

    if (!face_ok_ || (frame_no_ % kFaceEvery) != 0) {
        drop_stale();
        return;
    }
    cv::Mat gray, small;
    cv::cvtColor(frame, gray, cv::COLOR_BGR2GRAY);
    cv::resize(gray, small, cv::Size(), 0.5, 0.5);
    cv::equalizeHist(small, small);

    std::vector<cv::Rect> found;
    face_.detectMultiScale(small, found, 1.15, 4, 0, cv::Size(30, 30));
    if (found.empty() && profile_ok_) {
        // Head turned: the profile cascade sees one direction, so try both.
        profile_.detectMultiScale(small, found, 1.15, 4, 0, cv::Size(30, 30));
        if (found.empty()) {
            cv::Mat flipped;
            cv::flip(small, flipped, 1);
            std::vector<cv::Rect> f2;
            profile_.detectMultiScale(flipped, f2, 1.15, 4, 0, cv::Size(30, 30));
            for (auto r : f2) found.emplace_back(small.cols - r.x - r.width, r.y, r.width, r.height);
        }
    }
    if (!found.empty()) {
        faces_.clear();
        for (auto r : found) faces_.emplace_back(r.x * 2, r.y * 2, r.width * 2, r.height * 2);
        frames_since_face_ = 0;
        frames_face_held_ = 0;
    } else {
        drop_stale();
    }
}

// ── Lock-on tracking ────────────────────────────────────────

bool HandDetector::track(const cv::Mat& frame, HandObservation& obs, cv::Mat* overlay) {
    if (frame.empty()) return false;
    ++frame_no_;

    cv::Mat mask = skin_mask(frame);
    update_faces(frame, mask);

    // Erase faces from the skin mask. The cascade box is only the inner
    // face: widen it to cover forehead/hairline, ears, jaw and neck, or the
    // leftover cheek/ear strips get mistaken for a hand.
    // An OVAL for the head (forehead, ears, jaw) plus a narrow column for the
    // neck. A rectangle also covered the empty corners beside the chin —
    // exactly where a hand is held — and cut the fingers off.
    const cv::Rect bounds(0, 0, frame.cols, frame.rows);
    masked_.clear();
    face_mask_ = cv::Mat::zeros(frame.size(), CV_8U);
    for (const auto& f : faces_) {
        cv::Point c(f.x + f.width / 2, f.y + f.height * 45 / 100);
        cv::ellipse(face_mask_, c, cv::Size(f.width * 68 / 100, f.height * 85 / 100), 0, 0, 360,
                    cv::Scalar(255), cv::FILLED);
        cv::Rect neck(f.x + f.width * 18 / 100, f.y + f.height * 95 / 100,
                      f.width * 64 / 100, f.height * 95 / 100);
        cv::rectangle(face_mask_, neck & bounds, cv::Scalar(255), cv::FILLED);
        masked_.push_back(cv::Rect(c.x - f.width * 68 / 100, c.y - f.height * 85 / 100,
                                   f.width * 136 / 100, f.height * 170 / 100) & bounds);
    }
    if (!faces_.empty()) {
        mask.setTo(0, face_mask_);
        cv::dilate(face_mask_, face_halo_,
                   cv::getStructuringElement(cv::MORPH_ELLIPSE, {9, 9}));
    } else {
        face_halo_.release();
    }

    const double frame_area = static_cast<double>(frame.cols) * frame.rows;
    const double diag = std::sqrt(frame_area);
    auto found = blobs(mask, frame_area);

    const Blob* hand = nullptr;
    const Blob* pending = nullptr;
    if (locked_) {
        // Follow ONLY the locked hand: nearest blob within the gate, of a
        // plausible size. A bigger blob elsewhere (face, arm) is ignored.
        double best = 1e18;
        for (const auto& b : found) {
            double d = dist(b.center, last_center_);
            // Compare with the hand's usual size IN THE SAME POSE (a fist is
            // ~half an open hand, so one shared reference can't catch merges).
            const bool open_like = b.openness >= 0.45f;
            double ref = open_like ? area_open_ : area_fist_;
            double lo = 0.55, hi = kMaxAreaRatio;
            if (ref <= 0) {   // pose not seen yet: estimate from the other one
                ref = open_like ? 2.2 * area_fist_ : 0.45 * area_open_;
                lo = 0.4; hi = 2.5;
            }
            double ratio = b.area / std::max(1.0, ref);
            if (d > kGate * diag || ratio < lo || ratio > hi) continue;
            if (b.palm_r > kMergedPalm * lock_palm_r_) continue;        // merged with something bigger
            // Next to the masked face the hand may be cut by the mask edge —
            // fine, the face part is erased. But a thin cheek/ear strip left
            // at the edge must not take over: require a hand-sized palm.
            if (mask_contact(b) > kMaxContactOpen && (b.palm_r < 0.6 * lock_palm_r_ || ratio < 0.5)) continue;
            if (d < best) { best = d; hand = &b; }
        }

        // Closing the fingers can change the contour area and centroid in a
        // single frame. The strict pose-aware gate above used to reject that
        // perfectly normal change, after which the detector reported no hand
        // for ~0.4 s and released the lock before the FSM ever saw a fist.
        // Recover only near the previous hand and only with a hand-sized blob;
        // the face itself is erased from the mask and large/contact-heavy
        // remnants are still rejected.
        if (!hand) {
            double recovery_best = 1e18;
            for (const auto& b : found) {
                const double d = dist(b.center, last_center_);
                const double ratio = b.area / std::max(1.0, last_area_);
                if (d > kRecoveryGate * diag
                    || ratio < kRecoveryMinArea || ratio > kRecoveryMaxArea
                    || b.palm_r > 2.0 * lock_palm_r_
                    || mask_contact(b) > kRecoveryMaxContact)
                    continue;
                if (d < recovery_best) {
                    recovery_best = d;
                    hand = &b;
                }
            }
        }
        if (!hand && ++lost_frames_ > kLostFrames) {
            locked_ = false;
            pending_frames_ = 0;
            event_ = "Hand lock released (hand gone, merged or too close to the face)";
        }
    } else {
        // Acquire: an unmistakable OPEN hand, or — only when the face is known
        // and masked — a fist-shaped blob (so "fist -> open" = DROP can start).
        const Blob* best_open = nullptr;
        const Blob* best_fist = nullptr;
        int rejected_near_face = 0;
        for (const auto& b : found) {
            const double contact = mask_contact(b);
            if (contact > kMaxContactOpen) { ++rejected_near_face; continue; }
            if (is_open_hand(b)) {
                if (!best_open || b.area > best_open->area) best_open = &b;
            } else if (contact == 0.0 && is_fist(b, frame_area)) {
                if (!best_fist || b.area > best_fist->area) best_fist = &b;
            }
        }
        const Blob* cand = best_open ? best_open : best_fist;
        if (cand) {
            const bool open = cand == best_open;
            // Must look like that for several consecutive frames at the same spot.
            if (pending_frames_ > 0 && open == pending_open_ &&
                dist(cand->center, pending_center_) < kGate * diag)
                ++pending_frames_;
            else
                pending_frames_ = 1;
            pending_center_ = cand->center;
            pending_open_ = open;
            pending = cand;

            if (pending_frames_ >= kConfirmFrames) {
                locked_ = true;
                hand = cand;
                lock_palm_r_ = cand->palm_r;
                area_open_ = open ? cand->area : 0;
                area_fist_ = open ? 0 : cand->area;
                pending_frames_ = 0;
                char buf[200];
                std::snprintf(buf, sizeof(buf),
                              "Hand lock acquired on %s (fingers=%d, gaps=%d, palm r=%.0f, area=%.1f%%, at %.0f,%.0f)",
                              open ? "OPEN hand" : "fist (face known)", cand->fingers, cand->gaps,
                              cand->palm_r, 100.0 * cand->area / frame_area, cand->center.x, cand->center.y);
                event_ = buf;
            }
        } else {
            pending_frames_ = 0;
            if (rejected_near_face > 0 && frame_no_ % 30 == 0)
                event_ = "[debug] Ignoring " + std::to_string(rejected_near_face) +
                         " skin blob(s) touching the face area";
        }
    }

    if (hand) {
        last_center_ = hand->center;
        last_area_ = hand->area;
        last_box_ = hand->box;
        lost_frames_ = 0;
        // Follow slow size changes (hand moving toward/away from the camera),
        // but not the open<->fist jumps, so the limits stay meaningful.
        double& ref = hand->openness >= 0.45f ? area_open_ : area_fist_;
        ref = ref <= 0 ? hand->area : 0.9 * ref + 0.1 * hand->area;
        lock_palm_r_ = 0.9f * lock_palm_r_ + 0.1f * hand->palm_r;

        obs.openness = hand->openness;
        obs.finger_gaps = hand->gaps;
        obs.fingers = hand->fingers;
        obs.confidence = static_cast<float>(std::min(1.0, hand->area / (0.25 * frame_area)));
        obs.cx = hand->center.x / frame.cols;
        obs.cy = hand->center.y / frame.rows;
    }

    if (overlay) {
        draw(*overlay, hand);
        if (!hand && pending) {
            cv::rectangle(*overlay, pending->box, {0, 200, 255}, 1);
            cv::putText(*overlay, cv::format("EL? %d/%d", pending_frames_, kConfirmFrames),
                        {pending->box.x, std::max(14, pending->box.y - 6)},
                        cv::FONT_HERSHEY_SIMPLEX, 0.5, {0, 200, 255}, 1);
        }
    }
    return hand != nullptr;
}

void HandDetector::draw(cv::Mat& img, const Blob* hand) const {
    const cv::Scalar grey(150, 150, 150), green(0, 200, 0), orange(0, 160, 255),
        yellow(0, 230, 230), cyan(230, 200, 0);

    if (!masked_.empty() && !face_mask_.empty()) {
        std::vector<std::vector<cv::Point>> outline;
        cv::findContours(face_mask_.clone(), outline, cv::RETR_EXTERNAL, cv::CHAIN_APPROX_SIMPLE);
        cv::drawContours(img, outline, -1, grey, 1);
    }
    for (const auto& r : masked_)
        cv::putText(img, "YUZ - yok sayiliyor", {r.x + 4, std::max(14, r.y + 16)},
                    cv::FONT_HERSHEY_SIMPLEX, 0.45, grey, 1);

    if (hand) {
        std::vector<std::vector<cv::Point>> c{hand->contour};
        cv::drawContours(img, c, 0, green, 2);
        // Palm circle, finger ring and where fingers cross it.
        cv::circle(img, hand->palm, int(hand->palm_r), cyan, 1);
        if (hand->ring_r > 0) cv::circle(img, hand->palm, int(hand->ring_r), yellow, 1);
        for (const auto& p : hand->finger_pts) cv::circle(img, p, 5, {0, 0, 255}, -1);

        // The "EL" tag that sticks to the locked hand.
        cv::rectangle(img, hand->box, green, 2);
        const std::string tag = is_open_hand(*hand)
            ? cv::format("EL - acik (%d parmak)", hand->fingers)
            : hand->openness < 0.32f ? "EL - yumruk" : "EL";
        int base = 0;
        cv::Size ts = cv::getTextSize(tag, cv::FONT_HERSHEY_SIMPLEX, 0.6, 2, &base);
        cv::Point tl(hand->box.x, std::max(0, hand->box.y - ts.height - 10));
        cv::rectangle(img, tl, tl + cv::Point(ts.width + 10, ts.height + 10), green, cv::FILLED);
        cv::putText(img, tag, tl + cv::Point(5, ts.height + 4), cv::FONT_HERSHEY_SIMPLEX, 0.6,
                    {255, 255, 255}, 2);
    } else if (locked_) {
        cv::rectangle(img, last_box_, orange, 1);
        cv::putText(img, "EL kayboldu...", {last_box_.x, std::max(14, last_box_.y - 6)},
                    cv::FONT_HERSHEY_SIMPLEX, 0.5, orange, 1);
    } else {
        cv::putText(img, "Elini ACIK goster (parmaklar ayrik)", {12, 56},
                    cv::FONT_HERSHEY_SIMPLEX, 0.6, {0, 220, 255}, 2);
    }
}

// ── Stateless measurement (tests) ───────────────────────────

bool HandDetector::detect(const cv::Mat& frame, float& openness, float& confidence,
                          cv::Mat* overlay) {
    if (frame.empty()) return false;
    const double frame_area = static_cast<double>(frame.cols) * frame.rows;
    auto found = blobs(skin_mask(frame), frame_area);
    if (found.empty()) return false;
    const Blob& b = *std::max_element(found.begin(), found.end(),
        [](const Blob& x, const Blob& y) { return x.area < y.area; });
    openness = b.openness;
    confidence = static_cast<float>(std::min(1.0, b.area / (0.25 * frame_area)));
    if (overlay) draw(*overlay, &b);
    return true;
}
