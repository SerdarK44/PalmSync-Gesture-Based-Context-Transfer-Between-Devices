#include "ipc_sender.hpp"
#include "log.hpp"
#include <chrono>
#include <cstdio>
#include <cstring>

IpcSender::IpcSender(const std::string& dest_addr, int dest_port)
    : dest_addr_(dest_addr), dest_port_(dest_port) {}

IpcSender::~IpcSender() {
    shutdown();
}

bool IpcSender::initialize() {
#ifdef _WIN32
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        EYE_LOG("[Eye] WSAStartup failed\n");
        return false;
    }
    sock_ = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (sock_ == INVALID_SOCKET) {
        EYE_LOG("[Eye] Socket creation failed\n");
        return false;
    }
#else
    sock_ = socket(AF_INET, SOCK_DGRAM, 0);
    if (sock_ < 0) {
        EYE_LOG("[Eye] Socket creation failed\n");
        return false;
    }
#endif

    std::memset(&dest_, 0, sizeof(dest_));
    dest_.sin_family = AF_INET;
    dest_.sin_port = htons(static_cast<unsigned short>(dest_port_));
    inet_pton(AF_INET, dest_addr_.c_str(), &dest_.sin_addr);

    initialized_ = true;
    EYE_LOG("[Eye] IPC sender ready -> %s:%d\n",
                 dest_addr_.c_str(), dest_port_);
    return true;
}

bool IpcSender::send_event(const char* event_type, double confidence) {
    if (!initialized_) return false;

    auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
                   std::chrono::system_clock::now().time_since_epoch())
                   .count();

    char buf[256];
    int len = std::snprintf(buf, sizeof(buf),
        R"({"event":"%s","timestamp_ms":%lld,"confidence":%.4f})",
        event_type, static_cast<long long>(now), confidence);

    int sent = sendto(sock_, buf, len, 0,
                      reinterpret_cast<struct sockaddr*>(&dest_),
                      sizeof(dest_));
    return sent == len;
}

bool IpcSender::send_grab(double confidence) {
    return send_event("Grab", confidence);
}

bool IpcSender::send_drop(double confidence) {
    return send_event("Drop", confidence);
}

void IpcSender::shutdown() {
    if (!initialized_) return;
#ifdef _WIN32
    closesocket(sock_);
    WSACleanup();
    sock_ = INVALID_SOCKET;
#else
    close(sock_);
    sock_ = -1;
#endif
    initialized_ = false;
}
