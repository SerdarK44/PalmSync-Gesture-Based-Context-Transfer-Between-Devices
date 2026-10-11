#undef NDEBUG
#include "metrics_replay.hpp"

#include <cassert>
#include <cstdio>
#include <string>

// Real guided recordings from 2026-10-11 (Logitech webcam, landmark backend,
// 3 reps each). They pin the pose thresholds to measured data.
static const std::string kDir = PALMSYNC_FIXTURE_DIR;

static int count(const std::vector<ReplayEvent>& events, GestureEvent e, const std::string& cue) {
    int n = 0;
    for (const auto& ev : events)
        if (ev.event == e && ev.cue == cue) ++n;
    return n;
}

static int count_cue(const std::vector<ReplayEvent>& events, const std::string& cue) {
    int n = 0;
    for (const auto& ev : events)
        if (ev.cue == cue) ++n;
    return n;
}

static void check_recording(const char* file) {
    const auto frames = load_metrics_csv(kDir + "/" + file);
    assert(!frames.empty());

    // The old classic band (fist < 0.42) reproduces the false GRAB that
    // fired while the hand was lowered after the run.
    const auto old_events = replay(frames, gesture_config_for(false, 0.40f));
    assert(count(old_events, GestureEvent::Grab, "done") >= 1);

    // The cues in a recording advanced on the ORIGINAL events, so a replayed
    // GRAB that fires a frame later lands in hold_fist: same attempt.
    const auto events = replay(frames, gesture_config_for(true, 0.40f));
    assert(count_cue(events, "done") == 0);
    assert(count(events, GestureEvent::Grab, "fist") +
           count(events, GestureEvent::Grab, "hold_fist") == 3);
    assert(count(events, GestureEvent::Drop, "open") == 3);
    std::printf("  PASS %s: 3/3 GRAB, 3/3 DROP, no event after the run\n", file);
}

int main() {
    std::printf("Replay tests\n");
    check_recording("guided-0.5m-bright.csv");
    check_recording("guided-1.5m-bright.csv");
    std::printf("All replay tests passed\n");
    return 0;
}
