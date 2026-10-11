#include "hand_backend.hpp"

#include <algorithm>
#include <cctype>

bool parse_hand_backend(const std::string& value, HandBackend& backend) {
    std::string normalised = value;
    std::transform(normalised.begin(), normalised.end(), normalised.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });

    if (normalised == "auto") {
        backend = HandBackend::Auto;
        return true;
    }
    if (normalised == "landmark") {
        backend = HandBackend::Landmark;
        return true;
    }
    if (normalised == "classic") {
        backend = HandBackend::Classic;
        return true;
    }
    return false;
}

const char* hand_backend_name(HandBackend backend) {
    switch (backend) {
    case HandBackend::Auto: return "auto";
    case HandBackend::Landmark: return "landmark";
    case HandBackend::Classic: return "classic";
    }
    return "unknown";
}

HandBackendResolution resolve_hand_backend(HandBackend requested,
                                           bool landmark_available) {
    if (requested == HandBackend::Classic)
        return {true, HandBackend::Classic, "explicitly requested"};

    if (requested == HandBackend::Landmark) {
        if (landmark_available)
            return {true, HandBackend::Landmark, "explicitly requested"};
        return {false, HandBackend::Landmark,
                "landmark backend unavailable; models are missing, invalid, or failed to load"};
    }

    if (landmark_available)
        return {true, HandBackend::Landmark, "preferred backend available"};
    return {true, HandBackend::Classic,
            "landmark backend unavailable; using safe fallback"};
}

