#include "gesture_fsm.hpp"
#include <cassert>
#include <cstdio>
#include <thread>
#include <chrono>
#include <vector>

static std::vector<HandLandmark> make_open_hand() {
    std::vector<HandLandmark> lm(21);
    // Spread fingertips far from palm center
    for (int i = 0; i < 21; i++) {
        lm[i] = {static_cast<float>(i) * 0.05f,
                  static_cast<float>(i) * 0.05f, 0.0f, 0.99f};
    }
    return lm;
}

static std::vector<HandLandmark> make_closed_hand() {
    std::vector<HandLandmark> lm(21);
    // All landmarks clustered at the center
    for (int i = 0; i < 21; i++) {
        lm[i] = {0.5f, 0.5f, 0.0f, 0.99f};
    }
    return lm;
}

void test_closure_values() {
    GestureFSM fsm(0.3f, 50);

    float open_val = fsm.compute_closure(make_open_hand());
    float closed_val = fsm.compute_closure(make_closed_hand());

    std::printf("open_closure=%.4f  closed_closure=%.4f\n", open_val, closed_val);
    assert(closed_val < open_val && "closed hand should have smaller closure");
    std::printf("PASS: test_closure_values\n");
}

void test_grab_event() {
    GestureFSM fsm(0.3f, 50);
    int grab_count = 0;

    fsm.set_callback([&](GestureEvent e) {
        if (e == GestureEvent::Grab) grab_count++;
    });

    // Start open, then close
    fsm.update(make_open_hand());
    fsm.update(make_closed_hand());
    std::this_thread::sleep_for(std::chrono::milliseconds(60));
    fsm.update(make_closed_hand());

    assert(grab_count == 1 && "should fire exactly one Grab");
    std::printf("PASS: test_grab_event\n");
}

void test_debounce() {
    GestureFSM fsm(0.3f, 200);
    int event_count = 0;

    fsm.set_callback([&](GestureEvent) { event_count++; });

    // Rapid close updates within debounce window
    for (int i = 0; i < 5; i++) {
        fsm.update(make_closed_hand());
    }

    assert(event_count <= 1 && "debounce should prevent multiple events");
    std::printf("PASS: test_debounce\n");
}

int main() {
    test_closure_values();
    test_grab_event();
    test_debounce();
    std::printf("\nAll gesture tests passed.\n");
    return 0;
}
