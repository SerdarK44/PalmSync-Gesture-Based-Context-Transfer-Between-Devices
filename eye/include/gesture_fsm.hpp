#pragma once

#include <cstdint>
#include <functional>

enum class GestureEvent { Grab, Drop, Neutral };

// What the hand looks like in the current frame, after hysteresis.
enum class HandPose { None, Fist, Open, Unknown };

struct GestureConfig {
    float fist_threshold = 0.32f;  // openness below this  -> FIST
    float open_threshold = 0.52f;  // openness above this  -> OPEN (between = Unknown)
    int   prime_ms   = 65;         // two stable frames prime the first pose at 30 fps
    int   hold_ms    = 120;        // roughly 3-4 frames: fast but still rejects one-frame noise
    int   transition_window_ms = 900;   // deliberate transitions should be compact
    int   absent_rearm_ms = 250;   // hand out of view briefly -> ready for the next device
    int   latch_ms   = 1400;       // prevent an immediate inverse event after a gesture
    float max_jump   = 0.25f;      // centroid jump (normalised) -> it's a different object
    float max_motion = 0.035f;     // sustained per-frame motion makes pose measurements unreliable
    // Blend each frame's openness with the previous one (0.15 old / 0.85 new)
    // before classifying. The noisy classic detector needs it; for the
    // landmark detector it only delays every gesture by one frame.
    bool smooth_openness = true;
};

// Pose thresholds for a detector backend. The classic contour detector spreads
// poses over the whole 0..1 range, so it uses the --threshold boundary. The
// landmark detector reads a real fist as ~0.0 and an open hand as ~1.0; a
// hand being lowered or turned away reads ~0.2 (thumb out, other fingers
// foreshortened), which the classic boundary mistook for a fist (false GRAB
// after the 2026-10-11 camera runs). Its band is therefore much narrower,
// and the transition must be quicker (lowering a hand is a slow close).
GestureConfig gesture_config_for(bool landmark, float classic_threshold);

// Turns a per-frame "how open is the hand" signal into deliberate GRAB / DROP
// events.
//
// The old FSM fired on *every* fist and *every* open hand it saw, so a single
// fist-then-release produced GRAB+DROP, holding a hand in view produced a
// stream of events, and a face (a compact skin blob = "fist") kept triggering.
// This FSM only fires on a deliberate TRANSITION between two steady poses:
//
//   OPEN  (steady) -> FIST (held)   = GRAB   ("grab it")
//   FIST  (steady) -> OPEN (held)   = DROP   ("let it go")
//
// After an event it is LATCHED: nothing fires again until the hand leaves the
// view (or a different object appears, or `latch_ms` passes). So releasing
// the fist after a GRAB does not immediately DROP on the same machine.
//
// Time is injected (`now_ms`) so the behaviour is deterministic in tests.
class GestureFSM {
public:
    using Callback = std::function<void(GestureEvent)>;

    explicit GestureFSM(GestureConfig cfg = {});

    // Feed one frame. `cx`, `cy` are the hand centroid normalised to [0,1].
    void update(bool hand_present, float openness, float cx, float cy, int64_t now_ms);

    void set_callback(Callback cb);

    HandPose classify(float openness) const;

    HandPose pose() const { return stable_pose_; }
    bool armed() const { return armed_; }
    const GestureConfig& config() const { return cfg_; }

private:
    GestureConfig cfg_;
    Callback callback_;

    bool armed_ = true;
    int64_t last_event_ms_ = 0;

    bool have_track_ = false;
    float last_cx_ = 0.0f, last_cy_ = 0.0f;
    int motion_frames_ = 0;
    int64_t absent_since_ms_ = -1;

    HandPose candidate_ = HandPose::None;   // pose seen in recent frames
    int64_t candidate_since_ms_ = 0;
    HandPose stable_pose_ = HandPose::None; // last pose that was steady >= prime_ms
    int64_t stable_last_seen_ms_ = 0;       // last frame that still showed stable_pose_
    HandPose prev_pose_ = HandPose::None;   // steady pose before stable_pose_
    int64_t prev_end_ms_ = 0;               // when prev_pose_ ended

    void reset_track();
    void rearm();
    void emit(GestureEvent event, int64_t now_ms);
};
