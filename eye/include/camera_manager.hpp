#pragma once

#include <opencv2/videoio.hpp>
#include <opencv2/core.hpp>

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

private:
    cv::VideoCapture cap_;
    int camera_id_;
    int fps_;
    bool initialized_ = false;
};
