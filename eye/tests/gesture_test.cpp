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

void test_full_grab_drop_cycle() {
    GestureFSM fsm(0.3f, 50);
    std::vector<GestureEvent> events;

    fsm.set_callback([&](GestureEvent e) {
        events.push_back(e);
    });

    // Open -> Closing -> wait debounce -> GRAB (Closed)
    fsm.update(make_open_hand());
    fsm.update(make_closed_hand());
    std::this_thread::sleep_for(std::chrono::milliseconds(60));
    fsm.update(make_closed_hand());

    assert(events.size() == 1 && "should have one GRAB event");
    assert(events[0] == GestureEvent::Grab);

    // Closed -> Opening -> wait debounce -> DROP (Open)
    fsm.update(make_open_hand());
    std::this_thread::sleep_for(std::chrono::milliseconds(60));
    fsm.update(make_open_hand());

    assert(events.size() == 2 && "should have GRAB + DROP");
    assert(events[1] == GestureEvent::Drop);

    std::printf("PASS: test_full_grab_drop_cycle\n");
}

void test_insufficient_landmarks() {
    GestureFSM fsm(0.3f, 50);
    int event_count = 0;
    fsm.set_callback([&](GestureEvent) { event_count++; });

    // Fewer than 21 landmarks — should be ignored
    std::vector<HandLandmark> partial(10, {0.5f, 0.5f, 0.0f, 0.99f});
    fsm.update(partial);

    assert(event_count == 0 && "partial landmarks should not trigger events");
    std::printf("PASS: test_insufficient_landmarks\n");
}

void test_update_closure_direct() {
    // The classical detector feeds openness directly via update_closure.
    // fist (low openness) -> GRAB, open hand (high) -> DROP.
    GestureFSM fsm(0.4f, 50);
    std::vector<GestureEvent> events;
    fsm.set_callback([&](GestureEvent e) { events.push_back(e); });

    fsm.update_closure(0.9f);  // open
    fsm.update_closure(0.1f);  // fist
    std::this_thread::sleep_for(std::chrono::milliseconds(60));
    fsm.update_closure(0.1f);  // still fist -> GRAB

    assert(events.size() == 1 && events[0] == GestureEvent::Grab);

    fsm.update_closure(0.9f);  // open again
    std::this_thread::sleep_for(std::chrono::milliseconds(60));
    fsm.update_closure(0.9f);  // still open -> DROP

    assert(events.size() == 2 && events[1] == GestureEvent::Drop);
    std::printf("PASS: test_update_closure_direct\n");
}

int main() {
    test_closure_values();
    test_grab_event();
    test_debounce();
    test_full_grab_drop_cycle();
    test_insufficient_landmarks();
    test_update_closure_direct();
    std::printf("\nAll gesture tests passed. (6/6)\n");
    return 0;
}
