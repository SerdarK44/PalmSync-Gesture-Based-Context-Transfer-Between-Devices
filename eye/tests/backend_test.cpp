#undef NDEBUG
#include "hand_backend.hpp"

#include <cassert>
#include <cstdio>

int main() {
    HandBackend backend = HandBackend::Auto;
    assert(parse_hand_backend("auto", backend) && backend == HandBackend::Auto);
    assert(parse_hand_backend("LANDMARK", backend) && backend == HandBackend::Landmark);
    assert(parse_hand_backend("Classic", backend) && backend == HandBackend::Classic);
    assert(!parse_hand_backend("unknown", backend));

    auto automatic_fallback = resolve_hand_backend(HandBackend::Auto, false);
    assert(automatic_fallback.available);
    assert(automatic_fallback.selected == HandBackend::Classic);

    auto automatic_landmark = resolve_hand_backend(HandBackend::Auto, true);
    assert(automatic_landmark.available);
    assert(automatic_landmark.selected == HandBackend::Landmark);

    auto forced_missing = resolve_hand_backend(HandBackend::Landmark, false);
    assert(!forced_missing.available);

    auto forced_classic = resolve_hand_backend(HandBackend::Classic, false);
    assert(forced_classic.available);
    assert(forced_classic.selected == HandBackend::Classic);

    std::printf("All backend selection tests passed. (8/8)\n");
    return 0;
}

