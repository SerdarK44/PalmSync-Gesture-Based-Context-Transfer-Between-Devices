#include "camera_manager.hpp"
#include <iostream>

CameraManager::CameraManager(int camera_id, int fps)
    : camera_id_(camera_id), fps_(fps) {}

CameraManager::~CameraManager() {
    release();
}

bool CameraManager::initialize() {
    cap_.open(camera_id_);
    if (!cap_.isOpened()) {
        std::cerr << "[Eye] Failed to open camera " << camera_id_ << "\n";
        return false;
    }

    cap_.set(cv::CAP_PROP_FRAME_WIDTH, 640);
    cap_.set(cv::CAP_PROP_FRAME_HEIGHT, 480);
    cap_.set(cv::CAP_PROP_FPS, fps_);

    initialized_ = true;
    std::cerr << "[Eye] Camera " << camera_id_ << " initialized at "
              << fps_ << " FPS\n";
    return true;
}

bool CameraManager::capture_frame(cv::Mat& frame) {
    if (!initialized_) return false;
    return cap_.read(frame) && !frame.empty();
}

void CameraManager::release() {
    if (initialized_) {
        cap_.release();
        initialized_ = false;
    }
}

bool CameraManager::is_available() const {
    return initialized_ && cap_.isOpened();
}
