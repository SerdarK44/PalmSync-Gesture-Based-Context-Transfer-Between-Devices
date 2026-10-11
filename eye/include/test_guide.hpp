#pragma once

#include "gesture_fsm.hpp"

#include <cstdint>
#include <string>

// On-screen script for Camera 2.0 validation runs (--guide / --guide-empty).
//
// The FSM latches for latch_ms after every gesture, so a person who opens the
// hand right after a GRAB never produces the DROP. The guide therefore only
// asks for the next gesture once the FSM is armed again and the expected
// pose is steady; every cue that is not answered in time is recorded as a
// miss so analyze-metrics.ps1 can compute success rates from the CSV alone.
//
// Time is injected (`now_ms`) so the sequence is deterministic in tests.
enum class GuideStep {
    Countdown,  // get ready
    ShowOpen,   // show an open hand and keep it steady (first rep only)
    CueFist,    // close the hand now -> expect GRAB
    HoldFist,   // keep the fist until the FSM re-arms
    CueOpen,    // open the hand now -> expect DROP
    HoldOpen,   // keep the hand open until the FSM re-arms
    Empty,      // no hand in view for the whole duration
    Done
};

struct GuideConfig {
    int reps = 20;             // open -> fist -> open cycles
    int empty_seconds = 0;     // > 0 switches to the empty-scene run
    int countdown_ms = 3000;
    int cue_timeout_ms = 5000; // no gesture by then = miss
    int done_linger_ms = 2500; // keep the result on screen before exiting
};

class TestGuide {
public:
    explicit TestGuide(GuideConfig cfg);

    // Feed one frame after GestureFSM::update. `event` is "GRAB", "DROP" or
    // empty. Returns "MISS_GRAB" / "MISS_DROP" when a cue timed out on this
    // frame, otherwise an empty string.
    std::string update(int64_t now_ms, bool hand_present, HandPose stable_pose,
                       bool fsm_armed, const std::string& event);

    GuideStep step() const { return step_; }
    const char* step_name() const;   // CSV `cue` column
    bool finished(int64_t now_ms) const;

    // Display text, ASCII only (OpenCV's Hershey fonts have no Turkish glyphs).
    std::string headline(int64_t now_ms) const;
    std::string hint(bool hand_present, HandPose stable_pose, bool fsm_armed) const;
    // 0..1 progress of the current timed step, -1 when the step is untimed.
    float progress(int64_t now_ms) const;

    int rep() const { return rep_; }
    int reps() const { return cfg_.reps; }
    int grab_ok() const { return grab_ok_; }
    int grab_miss() const { return grab_miss_; }
    int drop_ok() const { return drop_ok_; }
    int drop_miss() const { return drop_miss_; }
    int wrong() const { return wrong_; }
    std::string summary() const;

private:
    void enter(GuideStep step, int64_t now_ms);

    GuideConfig cfg_;
    GuideStep step_ = GuideStep::Countdown;
    bool started_ = false;
    int64_t step_since_ms_ = 0;
    int64_t start_ms_ = 0;
    int rep_ = 0;
    int grab_ok_ = 0, grab_miss_ = 0;
    int drop_ok_ = 0, drop_miss_ = 0;
    int wrong_ = 0;
};
