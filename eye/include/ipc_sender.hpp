#pragma once

#include <string>

#ifdef _WIN32
#include <winsock2.h>
#pragma comment(lib, "ws2_32.lib")
#else
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#endif

class IpcSender {
public:
    explicit IpcSender(const std::string& dest_addr = "127.0.0.1", int dest_port = 12345);
    ~IpcSender();

    IpcSender(const IpcSender&) = delete;
    IpcSender& operator=(const IpcSender&) = delete;

    bool initialize();
    bool send_grab(double confidence);
    bool send_drop(double confidence);
    void shutdown();

private:
    std::string dest_addr_;
    int dest_port_;
#ifdef _WIN32
    SOCKET sock_ = INVALID_SOCKET;
#else
    int sock_ = -1;
#endif
    struct sockaddr_in dest_{};
    bool initialized_ = false;

    bool send_event(const char* event_type, double confidence);
};
