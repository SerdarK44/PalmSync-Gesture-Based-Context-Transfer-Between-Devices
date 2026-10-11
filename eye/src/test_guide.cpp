#include "test_guide.hpp"

#include <algorithm>
#include <cstdio>

namespace {
// A hold step that never settles (pose not recognised) moves on anyway so a
// bad scenario still finishes; the stuck marker shows up in the CSV.
constexpr int kHoldTimeoutMs = 8000;
}

TestGuide::TestGuide(GuideConfig cfg) : cfg_(cfg) {}

void TestGuide::enter(GuideStep step, int64_t now_ms) {
    step_ = step;
    step_since_ms_ = now_ms;
}

std::string TestGuide::update(int64_t now_ms, bool hand_present,
                              HandPose stable_pose, bool fsm_armed,
                              const std::string& event) {
    if (!started_) {
        started_ = true;
        start_ms_ = now_ms;
        enter(GuideStep::Countdown, now_ms);
    }
    const int64_t in_step = now_ms - step_since_ms_;

    if (!event.empty()) {
        if (step_ == GuideStep::CueFist && event == "GRAB") {
            ++grab_ok_;
            enter(GuideStep::HoldFist, now_ms);
        } else if (step_ == GuideStep::CueOpen && event == "DROP") {
            ++drop_ok_;
            ++rep_;
            enter(GuideStep::HoldOpen, now_ms);
        } else if (step_ != GuideStep::Done) {
            ++wrong_;
        }
        return {};
    }

    const bool open_ready = hand_present && fsm_armed && stable_pose == HandPose::Open;
    const bool fist_ready = hand_present && fsm_armed && stable_pose == HandPose::Fist;

    switch (step_) {
    case GuideStep::Countdown:
        if (in_step >= cfg_.countdown_ms)
            enter(cfg_.empty_seconds > 0 ? GuideStep::Empty : GuideStep::ShowOpen, now_ms);
        break;
    case GuideStep::ShowOpen:
        if (open_ready) enter(GuideStep::CueFist, now_ms);
        break;
    case GuideStep::CueFist:
        if (in_step >= cfg_.cue_timeout_ms) {
            ++grab_miss_;
            enter(GuideStep::HoldFist, now_ms);
            return "MISS_GRAB";
        }
        break;
    case GuideStep::HoldFist:
        if (fist_ready || in_step >= kHoldTimeoutMs) {
            const bool stuck = !fist_ready;
            enter(GuideStep::CueOpen, now_ms);
            if (stuck) return "STUCK_FIST";
        }
        break;
    case GuideStep::CueOpen:
        if (in_step >= cfg_.cue_timeout_ms) {
            ++drop_miss_;
            ++rep_;
            enter(GuideStep::HoldOpen, now_ms);
            return "MISS_DROP";
        }
        break;
    case GuideStep::HoldOpen:
        if (open_ready || in_step >= kHoldTimeoutMs) {
            const bool stuck = !open_ready;
            enter(rep_ >= cfg_.reps ? GuideStep::Done : GuideStep::CueFist, now_ms);
            if (stuck) return "STUCK_OPEN";
        }
        break;
    case GuideStep::Empty:
        if (in_step >= static_cast<int64_t>(cfg_.empty_seconds) * 1000)
            enter(GuideStep::Done, now_ms);
        break;
    case GuideStep::Done:
        break;
    }
    return {};
}

const char* TestGuide::step_name() const {
    switch (step_) {
    case GuideStep::Countdown: return "countdown";
    case GuideStep::ShowOpen:  return "show_open";
    case GuideStep::CueFist:   return "fist";
    case GuideStep::HoldFist:  return "hold_fist";
    case GuideStep::CueOpen:   return "open";
    case GuideStep::HoldOpen:  return "hold_open";
    case GuideStep::Empty:     return "empty";
    case GuideStep::Done:      return "done";
    }
    return "";
}

bool TestGuide::finished(int64_t now_ms) const {
    return step_ == GuideStep::Done && now_ms - step_since_ms_ >= cfg_.done_linger_ms;
}

std::string TestGuide::headline(int64_t now_ms) const {
    const int64_t in_step = now_ms - step_since_ms_;
    char buf[64];
    switch (step_) {
    case GuideStep::Countdown: {
        const int left = static_cast<int>((cfg_.countdown_ms - in_step + 999) / 1000);
        std::snprintf(buf, sizeof(buf), "HAZIRLAN  %d", std::max(1, left));
        return buf;
    }
    case GuideStep::ShowOpen:  return "ELINI AC VE SABIT TUT";
    case GuideStep::CueFist:   return "SIMDI: YUMRUK YAP";
    case GuideStep::HoldFist:  return "YUMRUGU TUT";
    case GuideStep::CueOpen:   return "SIMDI: ELINI AC";
    case GuideStep::HoldOpen:  return "ELINI ACIK TUT";
    case GuideStep::Empty: {
        const int64_t left_s = std::max<int64_t>(
            0, (static_cast<int64_t>(cfg_.empty_seconds) * 1000 - in_step + 999) / 1000);
        std::snprintf(buf, sizeof(buf), "EL GOSTERME  %d:%02d",
                      static_cast<int>(left_s / 60), static_cast<int>(left_s % 60));
        return buf;
    }
    case GuideStep::Done:      return "TAMAMLANDI";
    }
    return {};
}

std::string TestGuide::hint(bool hand_present, HandPose stable_pose,
                            bool fsm_armed) const {
    char buf[96];
    switch (step_) {
    case GuideStep::Countdown:
        return cfg_.empty_seconds > 0 ? "Elini kameradan uzak tut"
                                      : "Elini kadraja getir, avucun kameraya baksin";
    case GuideStep::Empty:
        return "Kamerada el olmasin; yuzun gorunebilir, normal davran";
    case GuideStep::Done:
        if (cfg_.empty_seconds > 0) {
            std::snprintf(buf, sizeof(buf), "Yanlis olay: %d", wrong_);
        } else {
            std::snprintf(buf, sizeof(buf), "GRAB %d/%d   DROP %d/%d   hatali %d",
                          grab_ok_, cfg_.reps, drop_ok_, cfg_.reps, wrong_);
        }
        return buf;
    default:
        break;
    }
    if (!hand_present) return "ELIN GORUNMUYOR - kameraya goster";
    switch (step_) {
    case GuideStep::ShowOpen:
    case GuideStep::HoldOpen:
        if (stable_pose != HandPose::Open) return "Parmaklarini tam ac";
        if (!fsm_armed) return "Bekle... elini acik tut";
        return {};
    case GuideStep::HoldFist:
        if (stable_pose != HandPose::Fist) return "Parmaklarini tam kapat";
        if (!fsm_armed) return "Bekle... yumrugu acma";
        return {};
    case GuideStep::CueFist:
    case GuideStep::CueOpen:
        return "Tek hareketle yap, elini kaydirma";
    default:
        return {};
    }
}

float TestGuide::progress(int64_t now_ms) const {
    const float in_step = static_cast<float>(now_ms - step_since_ms_);
    float total = 0.0f;
    switch (step_) {
    case GuideStep::Countdown: total = static_cast<float>(cfg_.countdown_ms); break;
    case GuideStep::CueFist:
    case GuideStep::CueOpen:   total = static_cast<float>(cfg_.cue_timeout_ms); break;
    case GuideStep::Empty:     total = cfg_.empty_seconds * 1000.0f; break;
    default:                   return -1.0f;
    }
    return total > 0.0f ? std::min(1.0f, in_step / total) : -1.0f;
}

std::string TestGuide::summary() const {
    char buf[160];
    std::snprintf(buf, sizeof(buf),
                  "reps=%d/%d grab_ok=%d grab_miss=%d drop_ok=%d drop_miss=%d wrong=%d",
                  rep_, cfg_.reps, grab_ok_, grab_miss_, drop_ok_, drop_miss_, wrong_);
    return buf;
}
