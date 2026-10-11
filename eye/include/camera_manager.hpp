#pragma once

#include <opencv2/videoio.hpp>
#include <opencv2/core.hpp>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <mutex>
#include <string>
#include <thread>

class CameraManager {
public:
    explicit CameraManager(int camera_id = 0, int fps = 30);
    ~CameraManager();

    CameraManager(const CameraManager&) = delete;
    CameraManager& operator=(const CameraManager&) = delete;

    bool initialize();
    // The NEWEST frame not returned yet (waits up to 1 s for one). Frames
    // that arrived while the previous one was being processed are skipped,
    // so latency never builds up behind a slow frame.
    bool capture_frame(cv::Mat& frame,
                       std::chrono::steady_clock::time_point* captured_at = nullptr);
    void release();
    bool is_available() const;

    // What initialize() ended up with (for --probe / the system check).
    int index() const { return camera_id_; }
    const std::string& backend() const { return backend_; }
    bool monochrome() const { return mono_; }
    // Frames the driver delivered but processing skipped (it was busy).
    uint64_t skipped_frames() const { return skipped_; }

private:
    bool try_open(int idx, int api, const char* name, bool& is_mono);
    bool finish(int idx, const char* name, bool mono);
    void reader_loop();

    cv::VideoCapture cap_;
    int camera_id_;
    int fps_;
    bool initialized_ = false;
    std::string backend_;
    bool mono_ = false;

    // Reader thread -> processing loop: a single "latest frame" slot.
    std::thread reader_;
    std::atomic<bool> stop_{false};
    std::mutex mutex_;
    std::condition_variable ready_;
    cv::Mat latest_;
    std::chrono::steady_clock::time_point latest_at_{};
    uint64_t seq_ = 0;
    uint64_t consumed_ = 0;
    std::atomic<uint64_t> skipped_{0};
};
