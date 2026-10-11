#pragma once

#include "gesture_fsm.hpp"

#include <cstdint>
#include <string>
#include <vector>

// Re-runs a --metrics CSV through GestureFSM with a chosen configuration, so
// threshold changes can be checked against real camera recordings instead of
// guessed. The openness smoothing is recomputed exactly as main.cpp does it.
struct ReplayFrame {
    int64_t t_ms = 0;
    bool found = false;
    float openness = 0.0f;
    float cx = 0.5f, cy = 0.5f;  // older recordings have no centroid columns
    std::string cue;
};

struct ReplayEvent {
    int64_t t_ms = 0;
    GestureEvent event = GestureEvent::Neutral;
    std::string cue;  // guide cue active when it fired ("" for free runs)
};

// Throws std::runtime_error when the file cannot be read or lacks columns.
std::vector<ReplayFrame> load_metrics_csv(const std::string& path);

std::vector<ReplayEvent> replay(const std::vector<ReplayFrame>& frames,
                                const GestureConfig& cfg);
