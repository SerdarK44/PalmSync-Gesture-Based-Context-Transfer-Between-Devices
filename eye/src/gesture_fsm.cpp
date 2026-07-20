#include "gesture_fsm.hpp"
#include <cmath>
#include <numeric>

GestureFSM::GestureFSM(float close_threshold, int debounce_ms)
    : close_threshold_(close_threshold),
      debounce_ms_(debounce_ms),
      last_event_time_(std::chrono::steady_clock::now()) {}

void GestureFSM::set_callback(Callback cb) {
    callback_ = std::move(cb);
}

float GestureFSM::compute_closure(const std::vector<HandLandmark>& lm) const {
    if (lm.size() < 21) return 1.0f;

    // Palm center: average of wrist(0), index_mcp(5), pinky_mcp(17)
    float cx = (lm[0].x + lm[5].x + lm[17].x) / 3.0f;
    float cy = (lm[0].y + lm[5].y + lm[17].y) / 3.0f;

    // Fingertip indices: thumb(4), index(8), middle(12), ring(16), pinky(20)
    constexpr int tips[] = {4, 8, 12, 16, 20};
    float total = 0.0f;
    for (int i : tips) {
        float dx = lm[i].x - cx;
        float dy = lm[i].y - cy;
        total += std::sqrt(dx * dx + dy * dy);
    }

    return total / 5.0f;
}

void GestureFSM::update(const std::vector<HandLandmark>& landmarks) {
    if (landmarks.size() < 21) return;
    update_closure(compute_closure(landmarks));
}

void GestureFSM::update_closure(float closure) {
    switch (state_) {
        case State::Open:
            if (closure < close_threshold_) {
                state_ = State::Closing;
            }
            break;

        case State::Closing:
            if (closure >= close_threshold_) {
                state_ = State::Open;
            } else if (debounce_elapsed()) {
                emit(GestureEvent::Grab);
                state_ = State::Closed;
            }
            break;

        case State::Closed:
            if (closure >= close_threshold_) {
                state_ = State::Opening;
            }
            break;

        case State::Opening:
            if (closure < close_threshold_) {
                state_ = State::Closed;
            } else if (debounce_elapsed()) {
                emit(GestureEvent::Drop);
                state_ = State::Open;
            }
            break;
    }
}

void GestureFSM::emit(GestureEvent event) {
    last_event_time_ = std::chrono::steady_clock::now();
    if (callback_) {
        callback_(event);
    }
}

bool GestureFSM::debounce_elapsed() const {
    auto now = std::chrono::steady_clock::now();
    auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
                       now - last_event_time_)
                       .count();
    return elapsed >= debounce_ms_;
}
