// palmsync-eye-replay <metrics.csv> [--classic <threshold>]
//
// Prints the gestures the current GestureFSM would emit for a recorded
// --metrics CSV, with the guide cue each one landed in.
#include "metrics_replay.hpp"

#include <cstdio>
#include <cstdlib>
#include <exception>
#include <map>
#include <string>

int main(int argc, char** argv) {
    if (argc < 2) {
        std::fprintf(stderr, "usage: palmsync-eye-replay <metrics.csv> [--classic <threshold>]\n");
        return 2;
    }
    bool landmark = true;
    float threshold = 0.40f;
    if (argc >= 4 && std::string(argv[2]) == "--classic") {
        landmark = false;
        threshold = static_cast<float>(std::atof(argv[3]));
    }
    try {
        const auto cfg = gesture_config_for(landmark, threshold);
        const auto events = replay(load_metrics_csv(argv[1]), cfg);
        std::map<std::string, int> per_cue;
        for (const auto& e : events) {
            const char* name = e.event == GestureEvent::Grab ? "GRAB" : "DROP";
            std::printf("%8lld ms  %s  cue=%s\n", static_cast<long long>(e.t_ms), name,
                        e.cue.empty() ? "-" : e.cue.c_str());
            ++per_cue[std::string(name) + "@" + (e.cue.empty() ? "-" : e.cue)];
        }
        std::printf("thresholds fist<%.2f open>%.2f, %zu events:", cfg.fist_threshold,
                    cfg.open_threshold, events.size());
        for (const auto& [key, n] : per_cue) std::printf(" %s=%d", key.c_str(), n);
        std::printf("\n");
    } catch (const std::exception& ex) {
        std::fprintf(stderr, "error: %s\n", ex.what());
        return 1;
    }
    return 0;
}
