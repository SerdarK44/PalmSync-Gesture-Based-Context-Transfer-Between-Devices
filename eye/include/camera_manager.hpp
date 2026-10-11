#pragma once

#include <opencv2/videoio.hpp>
#include <opencv2/core.hpp>

#include <string>

class CameraManager {
public:
    explicit CameraManager(int camera_id = 0, int fps = 30);
    ~CameraManager();

    CameraManager(const CameraManager&) = delete;
    CameraManager& operator=(const CameraManager&) = delete;

    bool initialize();
    bool capture_frame(cv::Mat& frame);
    void release();
    bool is_available() const;

    // What initialize() ended up with (for --probe / the system check).
    int index() const { return camera_id_; }
    const std::string& backend() const { return backend_; }
    bool monochrome() const { return mono_; }

private:
    bool try_open(int idx, int api, const char* name, bool& is_mono);
    bool finish(int idx, const char* name, bool mono);

    cv::VideoCapture cap_;
    int camera_id_;
    int fps_;
    bool initialized_ = false;
    std::string backend_;
    bool mono_ = false;
};
