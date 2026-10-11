#include "metrics_replay.hpp"

#include <fstream>
#include <map>
#include <sstream>
#include <stdexcept>

namespace {

std::vector<std::string> split_csv(const std::string& line) {
    std::vector<std::string> out;
    std::stringstream ss(line);
    std::string cell;
    while (std::getline(ss, cell, ',')) out.push_back(cell);
    if (!line.empty() && line.back() == ',') out.emplace_back();
    return out;
}

}  // namespace

std::vector<ReplayFrame> load_metrics_csv(const std::string& path) {
    std::ifstream in(path);
    if (!in) throw std::runtime_error("cannot open " + path);

    std::string line;
    if (!std::getline(in, line)) throw std::runtime_error("empty file " + path);
    if (!line.empty() && line.back() == '\r') line.pop_back();
    std::map<std::string, size_t> col;
    const auto header = split_csv(line);
    for (size_t i = 0; i < header.size(); ++i) col[header[i]] = i;
    for (const char* need : {"t_ms", "found", "openness"})
        if (!col.count(need)) throw std::runtime_error(std::string("missing column ") + need);

    auto get = [&](const std::vector<std::string>& cells, const char* name) -> std::string {
        auto it = col.find(name);
        return it != col.end() && it->second < cells.size() ? cells[it->second] : std::string();
    };

    std::vector<ReplayFrame> frames;
    while (std::getline(in, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        if (line.empty()) continue;
        const auto cells = split_csv(line);
        ReplayFrame f;
        f.t_ms = std::stoll(get(cells, "t_ms"));
        f.found = get(cells, "found") == "1";
        f.openness = std::stof(get(cells, "openness"));
        const std::string cx = get(cells, "cx"), cy = get(cells, "cy");
        if (!cx.empty() && !cy.empty()) {
            f.cx = std::stof(cx);
            f.cy = std::stof(cy);
        }
        f.cue = get(cells, "cue");
        frames.push_back(f);
    }
    return frames;
}

std::vector<ReplayEvent> replay(const std::vector<ReplayFrame>& frames,
                                const GestureConfig& cfg) {
    GestureFSM fsm(cfg);
    std::vector<ReplayEvent> events;
    const ReplayFrame* current = nullptr;
    fsm.set_callback([&](GestureEvent e) {
        events.push_back({current->t_ms, e, current->cue});
    });

    // Same smoothing as the live loop in main.cpp.
    float smoothed = 1.0f;
    bool have_hand = false;
    for (const auto& f : frames) {
        current = &f;
        if (f.found) {
            smoothed = have_hand && cfg.smooth_openness
                ? (0.15f * smoothed + 0.85f * f.openness) : f.openness;
            have_hand = true;
        } else {
            have_hand = false;
        }
        fsm.update(f.found, smoothed, f.cx, f.cy, f.t_ms);
    }
    return events;
}
