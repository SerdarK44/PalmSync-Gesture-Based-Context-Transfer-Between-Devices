// Deterministic tests for GestureFSM. Time is simulated (no sleeps): each
// helper feeds frames at ~30 fps and advances a virtual clock.

// Tests are built in Release (WDAC blocks Debug exes) — keep assert() active.
#undef NDEBUG
#include "gesture_fsm.hpp"
#include <cassert>
#include <cstdio>
#include <vector>

namespace {

constexpr float kOpen = 0.90f;
constexpr float kFist = 0.05f;
constexpr int kFrameMs = 33;

struct Sim {
    GestureFSM fsm;
    std::vector<GestureEvent> events;
    int64_t t = 1000;

    Sim() {
        fsm.set_callback([this](GestureEvent e) { events.push_back(e); });
    }

    // Hold a pose for `ms` at position (x, y).
    void hold(float openness, int ms, float x = 0.5f, float y = 0.5f) {
        for (int e = 0; e < ms; e += kFrameMs) {
            fsm.update(true, openness, x, y, t);
            t += kFrameMs;
        }
    }

    void absent(int ms) {
        for (int e = 0; e < ms; e += kFrameMs) {
            fsm.update(false, 0.0f, 0.0f, 0.0f, t);
            t += kFrameMs;
        }
    }

    int count(GestureEvent ev) const {
        int n = 0;
        for (auto e : events) n += (e == ev);
        return n;
    }
};

}  // namespace

void test_open_then_fist_grabs_once() {
    Sim s;
    s.hold(kOpen, 500);
    s.hold(kFist, 2000);  // holding the fist must not repeat
    assert(s.events.size() == 1 && s.events[0] == GestureEvent::Grab);
    std::printf("PASS: test_open_then_fist_grabs_once\n");
}

void test_fist_then_open_drops_once() {
    Sim s;
    s.hold(kFist, 500);
    s.hold(kOpen, 2000);
    assert(s.events.size() == 1 && s.events[0] == GestureEvent::Drop);
    std::printf("PASS: test_fist_then_open_drops_once\n");
}

void test_static_blob_never_fires() {
    // A face reads as a compact "fist" forever; an open hand held still is
    // just "open". Neither is a transition, so nothing may fire.
    Sim a;
    a.hold(kFist, 10000);
    assert(a.events.empty());

    Sim b;
    b.hold(kOpen, 10000);
    assert(b.events.empty());
    std::printf("PASS: test_static_blob_never_fires\n");
}

void test_release_after_grab_does_not_drop() {
    // The old bug: fist (GRAB) then opening the hand immediately DROPped on
    // the same machine. Now the FSM is latched after GRAB.
    Sim s;
    s.hold(kOpen, 500);
    s.hold(kFist, 600);
    s.hold(kOpen, 1500);
    assert(s.count(GestureEvent::Grab) == 1);
    assert(s.count(GestureEvent::Drop) == 0);
    std::printf("PASS: test_release_after_grab_does_not_drop\n");
}

void test_hand_leaves_then_drop_works() {
    Sim s;
    s.hold(kOpen, 500);
    s.hold(kFist, 600);   // GRAB
    s.absent(800);        // hand leaves the camera -> re-arm
    s.hold(kFist, 400);   // comes back closed ...
    s.hold(kOpen, 600);   // ... and opens = DROP
    assert(s.events.size() == 2);
    assert(s.events[0] == GestureEvent::Grab && s.events[1] == GestureEvent::Drop);
    std::printf("PASS: test_hand_leaves_then_drop_works\n");
}

void test_flicker_is_ignored() {
    // Noisy detector alternating every frame must not produce events.
    Sim s;
    for (int i = 0; i < 300; i++) s.hold(i % 2 ? kOpen : kFist, kFrameMs);
    assert(s.events.empty());
    std::printf("PASS: test_flicker_is_ignored\n");
}

void test_short_fist_is_ignored() {
    Sim s;
    s.hold(kOpen, 500);
    s.hold(kFist, 200);   // too short to count
    s.hold(kOpen, 1000);
    assert(s.events.empty());
    std::printf("PASS: test_short_fist_is_ignored\n");
}

void test_object_switch_is_not_a_transition() {
    // Open hand on the right, then the hand leaves and a face (fist-like)
    // far away becomes the largest blob. That's not a GRAB.
    Sim s;
    s.hold(kOpen, 600, 0.85f, 0.6f);
    s.hold(kFist, 2000, 0.40f, 0.25f);
    assert(s.events.empty());
    std::printf("PASS: test_object_switch_is_not_a_transition\n");
}

void test_moving_hand_is_unknown() {
    // Hand sweeping across the frame while changing shape: no event.
    Sim s;
    float x = 0.1f;
    for (int i = 0; i < 15; i++) { s.fsm.update(true, kOpen, x, 0.5f, s.t); s.t += kFrameMs; x += 0.045f; }
    for (int i = 0; i < 15; i++) { s.fsm.update(true, kFist, x, 0.5f, s.t); s.t += kFrameMs; x -= 0.045f; }
    assert(s.events.empty());
    std::printf("PASS: test_moving_hand_is_unknown\n");
}

void test_latch_expires_for_new_gesture() {
    Sim s;
    s.hold(kOpen, 500);
    s.hold(kFist, 600);   // GRAB
    s.hold(kOpen, 3500);  // stays in view, latch expires, no DROP
    assert(s.events.size() == 1);
    s.hold(kFist, 600);   // a fresh open->fist transition = GRAB again
    assert(s.events.size() == 2 && s.events[1] == GestureEvent::Grab);
    std::printf("PASS: test_latch_expires_for_new_gesture\n");
}

void test_slow_transition_is_ignored() {
    // Open, then a long ambiguous pause, then fist: not a deliberate grab.
    Sim s;
    s.hold(kOpen, 500);
    s.hold(0.40f, 2500);  // in the hysteresis band = Unknown
    s.hold(kFist, 1000);
    assert(s.events.empty());
    std::printf("PASS: test_slow_transition_is_ignored\n");
}

void test_hysteresis_classification() {
    GestureFSM fsm;
    assert(fsm.classify(0.10f) == HandPose::Fist);
    assert(fsm.classify(0.40f) == HandPose::Unknown);
    assert(fsm.classify(0.80f) == HandPose::Open);
    std::printf("PASS: test_hysteresis_classification\n");
}

int main() {
    test_open_then_fist_grabs_once();
    test_fist_then_open_drops_once();
    test_static_blob_never_fires();
    test_release_after_grab_does_not_drop();
    test_hand_leaves_then_drop_works();
    test_flicker_is_ignored();
    test_short_fist_is_ignored();
    test_object_switch_is_not_a_transition();
    test_moving_hand_is_unknown();
    test_latch_expires_for_new_gesture();
    test_slow_transition_is_ignored();
    test_hysteresis_classification();
    std::printf("\nAll gesture tests passed. (12/12)\n");
    return 0;
}
