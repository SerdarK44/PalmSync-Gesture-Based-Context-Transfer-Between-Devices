#include "gesture_fsm.hpp"
#include <cmath>

GestureConfig gesture_config_for(bool landmark, float classic_threshold) {
    GestureConfig cfg;
    if (landmark) {
        cfg.fist_threshold = 0.15f;
        cfg.open_threshold = 0.60f;
        // Measured open<->fist gaps: 63-400 ms for real gestures, 591 ms for
        // a hand being lowered and closed, ~1 s while getting into position.
        cfg.transition_window_ms = 550;
        // Landmark poses don't flicker frame to frame like skin blobs do:
        // two consecutive frames are enough, and no smoothing lag.
        cfg.prime_ms = 50;
        cfg.hold_ms = 60;
        cfg.smooth_openness = false;
    } else {
        cfg.fist_threshold = classic_threshold + 0.02f;
        cfg.open_threshold = classic_threshold + 0.12f;
    }
    return cfg;
}

GestureFSM::GestureFSM(GestureConfig cfg) : cfg_(cfg) {}

void GestureFSM::set_callback(Callback cb) {
    callback_ = std::move(cb);
}

HandPose GestureFSM::classify(float openness) const {
    if (openness < cfg_.fist_threshold) return HandPose::Fist;
    if (openness > cfg_.open_threshold) return HandPose::Open;
    return HandPose::Unknown;
}

void GestureFSM::reset_track() {
    candidate_ = HandPose::None;
    stable_pose_ = HandPose::None;
    prev_pose_ = HandPose::None;
    motion_frames_ = 0;
}

void GestureFSM::rearm() {
    armed_ = true;
    // A new transition must start from scratch after re-arming.
    prev_pose_ = HandPose::None;
}

void GestureFSM::update(bool hand_present, float openness, float cx, float cy,
                        int64_t now_ms) {
    if (!armed_ && now_ms - last_event_ms_ >= cfg_.latch_ms) rearm();

    // ── Hand gone ─────────────────────────────────────────
    if (!hand_present) {
        if (absent_since_ms_ < 0) absent_since_ms_ = now_ms;
        if (now_ms - absent_since_ms_ >= cfg_.absent_rearm_ms) {
            reset_track();
            have_track_ = false;
            if (!armed_) rearm();
        }
        return;
    }
    absent_since_ms_ = -1;

    // ── Continuity: is this the same object as last frame? ──
    HandPose p = classify(openness);
    if (have_track_) {
        float dx = cx - last_cx_, dy = cy - last_cy_;
        float d = std::sqrt(dx * dx + dy * dy);
        if (d > cfg_.max_jump) {
            // Largest skin blob switched (hand left, face took over, ...).
            reset_track();
            if (!armed_) rearm();
        } else if (d > cfg_.max_motion) {
            // A single centroid jump is normal when fingers fold into the
            // palm. Only sustained motion means the shape measurement is
            // unreliable (for example, waving across the frame).
            if (++motion_frames_ >= 3) p = HandPose::Unknown;
        } else {
            motion_frames_ = 0;
        }
    }
    have_track_ = true;
    last_cx_ = cx;
    last_cy_ = cy;

    // ── Pose stability ────────────────────────────────────
    if (p != candidate_) {
        candidate_ = p;
        candidate_since_ms_ = now_ms;
        return;
    }
    if (p == HandPose::Unknown || p == HandPose::None) return;

    if (p == stable_pose_) stable_last_seen_ms_ = now_ms;

    int64_t held = now_ms - candidate_since_ms_;
    // The first pose only primes the gesture. A different pose must satisfy
    // the full hold time before it becomes stable; otherwise a brief noisy
    // fist could silently become the previous pose and trigger DROP when the
    // hand returns to open.
    const int required_ms = stable_pose_ == HandPose::None
        ? cfg_.prime_ms : cfg_.hold_ms;
    if (held < required_ms) return;

    if (p != stable_pose_) {
        if (stable_pose_ != HandPose::None) {
            prev_pose_ = stable_pose_;
            prev_end_ms_ = stable_last_seen_ms_;
        }
        stable_pose_ = p;
        stable_last_seen_ms_ = now_ms;
    }

    // ── Fire on a deliberate transition ───────────────────
    if (!armed_ || held < cfg_.hold_ms) return;
    if (prev_pose_ == HandPose::None || prev_pose_ == stable_pose_) return;
    if (candidate_since_ms_ - prev_end_ms_ > cfg_.transition_window_ms) return;

    if (prev_pose_ == HandPose::Open && stable_pose_ == HandPose::Fist)
        emit(GestureEvent::Grab, now_ms);
    else if (prev_pose_ == HandPose::Fist && stable_pose_ == HandPose::Open)
        emit(GestureEvent::Drop, now_ms);
}

void GestureFSM::emit(GestureEvent event, int64_t now_ms) {
    armed_ = false;
    last_event_ms_ = now_ms;
    prev_pose_ = HandPose::None;
    if (callback_) callback_(event);
}
