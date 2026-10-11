#pragma once

#include <string>

enum class HandBackend {
    Auto,
    Landmark,
    Classic,
};

struct HandBackendResolution {
    bool available = false;
    HandBackend selected = HandBackend::Classic;
    std::string reason;
};

bool parse_hand_backend(const std::string& value, HandBackend& backend);
const char* hand_backend_name(HandBackend backend);
HandBackendResolution resolve_hand_backend(HandBackend requested,
                                           bool landmark_available);

