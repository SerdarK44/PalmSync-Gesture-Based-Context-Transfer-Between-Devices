#undef NDEBUG
#include "gesture_fsm.hpp"
#include "test_guide.hpp"

#include <cassert>
#include <cstdio>
#include <string>

// Drives the real GestureFSM and the guide together, the same way main.cpp
// does, with a simulated person who follows (or ignores) the cues.
struct Rig {
    GestureFSM fsm;
    TestGuide guide;
    int64_t now = 0;
    std::string event;
    std::string last_marker;

    explicit Rig(GuideConfig cfg) : guide(cfg) {
        fsm.set_callback([this](GestureEvent e) {
            event = e == GestureEvent::Grab ? "GRAB" : "DROP";
        });
    }

    // One 30 fps frame with the given openness (< 0 = no hand).
    void frame(float openness) {
        now += 33;
        event.clear();
        const bool hand = openness >= 0.0f;
        fsm.update(hand, hand ? openness : 0.0f, 0.5f, 0.5f, now);
        const std::string marker = guide.update(now, hand, fsm.pose(), fsm.armed(), event);
        if (!marker.empty()) last_marker = marker;
    }

    // A cooperative person: open hand for open cues, fist for fist cues.
    void follow(int frames) {
        for (int i = 0; i < frames; ++i) {
            const GuideStep s = guide.step();
            const bool fist = s == GuideStep::CueFist || s == GuideStep::HoldFist;
            frame(fist ? 0.05f : 0.95f);
        }
    }
};

static void test_cooperative_run_counts_every_rep() {
    GuideConfig cfg;
    cfg.reps = 3;
    Rig rig(cfg);
    rig.follow(30 * 30);  // 30 s is plenty for 3 reps with latch waits
    assert(rig.guide.step() == GuideStep::Done);
    assert(rig.guide.grab_ok() == 3 && rig.guide.drop_ok() == 3);
    assert(rig.guide.grab_miss() == 0 && rig.guide.drop_miss() == 0);
    assert(rig.guide.wrong() == 0);
    assert(rig.guide.rep() == 3);
    std::printf("  PASS cooperative run: %s\n", rig.guide.summary().c_str());
}

static void test_ignored_cue_is_a_miss() {
    GuideConfig cfg;
    cfg.reps = 1;
    Rig rig(cfg);
    while (rig.guide.step() != GuideStep::CueFist) rig.frame(0.95f);
    // Person keeps the hand open: the GRAB cue must time out.
    for (int i = 0; i < 6 * 30 && rig.guide.step() == GuideStep::CueFist; ++i)
        rig.frame(0.95f);
    assert(rig.guide.grab_miss() == 1 && rig.last_marker == "MISS_GRAB");
    assert(rig.guide.step() == GuideStep::HoldFist);
    std::printf("  PASS ignored fist cue recorded as MISS_GRAB\n");
}

static void test_opening_during_latch_waits_instead_of_missing() {
    GuideConfig cfg;
    cfg.reps = 1;
    Rig rig(cfg);
    while (rig.guide.step() != GuideStep::HoldFist) rig.follow(1);
    // Hand opens too early while the FSM is latched: the guide must stay on
    // HOLD FIST instead of cueing an open hand that cannot produce a DROP.
    for (int i = 0; i < 10; ++i) rig.frame(0.95f);
    assert(rig.guide.step() == GuideStep::HoldFist);
    assert(rig.guide.hint(true, rig.fsm.pose(), rig.fsm.armed()) == "Parmaklarini tam kapat");
    std::printf("  PASS early open keeps the HOLD FIST cue\n");
}

static void test_empty_scene_counts_wrong_events() {
    GuideConfig cfg;
    cfg.empty_seconds = 2;
    Rig rig(cfg);
    while (rig.guide.step() != GuideStep::Empty) rig.frame(-1.0f);
    assert(rig.guide.headline(rig.now).rfind("EL GOSTERME", 0) == 0);
    // A face mistaken for open -> fist would emit GRAB: counted as wrong.
    for (int i = 0; i < 10; ++i) rig.frame(0.95f);
    for (int i = 0; i < 10; ++i) rig.frame(0.05f);
    while (rig.guide.step() == GuideStep::Empty) rig.frame(-1.0f);
    assert(rig.guide.step() == GuideStep::Done && rig.guide.wrong() == 1);
    assert(!rig.guide.finished(rig.now));
    for (int i = 0; i < 100; ++i) rig.frame(-1.0f);
    assert(rig.guide.finished(rig.now));
    std::printf("  PASS empty scene counts wrong events and finishes\n");
}

int main() {
    std::printf("Test guide tests\n");
    test_cooperative_run_counts_every_rep();
    test_ignored_cue_is_a_miss();
    test_opening_during_latch_waits_instead_of_missing();
    test_empty_scene_counts_wrong_events();
    std::printf("All guide tests passed\n");
    return 0;
}
