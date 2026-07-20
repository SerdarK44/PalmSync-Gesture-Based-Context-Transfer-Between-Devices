#pragma once

#include <chrono>
#include <functional>
#include <vector>

struct HandLandmark {
    float x, y, z;
    float confidence;
};

enum class GestureEvent { Grab, Drop, Neutral };

class GestureFSM {
public:
    using Callback = std::function<void(GestureEvent)>;

    explicit GestureFSM(float close_threshold = 0.3f, int debounce_ms = 200);

    void update(const std::vector<HandLandmark>& landmarks);

    // Feed an openness value directly (0 = fist, 1 = fully open hand).
    // Used by the classical (model-free) detector.
    void update_closure(float closure);

    void set_callback(Callback cb);

    float compute_closure(const std::vector<HandLandmark>& landmarks) const;

private:
    enum class State { Open, Closing, Closed, Opening };

    State state_ = State::Open;
    float close_threshold_;
    int debounce_ms_;
    std::chrono::steady_clock::time_point last_event_time_;
    Callback callback_;

    void emit(GestureEvent event);
    bool debounce_elapsed() const;
};
