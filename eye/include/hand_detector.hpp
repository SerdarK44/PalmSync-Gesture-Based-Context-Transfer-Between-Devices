#pragma once

#include <opencv2/core.hpp>
#include <opencv2/objdetect.hpp>

#include <string>
#include <vector>

struct HandObservation {
    float openness = 0.0f;    // 0 = fist, 1 = fully spread
    float confidence = 0.0f;
    float cx = 0.0f, cy = 0.0f;  // blob centroid, normalised to [0,1]
    int finger_gaps = 0;      // convexity-defect valleys
    int fingers = 0;          // fingers crossing the ring around the palm
};

// Model-free hand detector with hand LOCK-ON tracking.
//
// The old detector took the LARGEST skin blob every frame — usually the face,
// which then read as a "fist". This one is strict about what a hand is:
//
//  1. Faces (frontal + profile Haar cascades) are erased from the skin mask
//     together with forehead, ears, jaw and neck; any blob that still touches
//     that area is ignored (cut-off cheek/ear strips).
//  2. An OPEN hand is recognised by the palm-circle method: the largest
//     circle inside the blob is the palm; a ring at ~1.6–2.2× its radius must
//     be crossed by ≥ 3 finger-wide strips (a face crosses it nowhere, a
//     face strip only with one wide band). Convexity-defect valleys (≥ 2)
//     must agree.
//  3. The lock is only taken after the candidate looked like that for several
//     consecutive frames at the same place (no one-frame flukes).
//  4. Once locked, ONLY that blob is followed (nearest, similar size); if its
//     palm suddenly grows (merged with the face) or it vanishes, the lock is
//     released.
class HandDetector {
public:
    explicit HandDetector(int min_area_percent = 1);

    // Cascade XML CONTENT (read by the caller with Unicode-safe file APIs).
    bool load_face_model(const std::string& xml);
    bool load_profile_model(const std::string& xml);
    bool has_face_model() const { return face_ok_; }

    // Stateful lock-on tracking (used by the camera loop). Returns true while
    // the locked hand is visible and fills `obs`.
    bool track(const cv::Mat& frame, HandObservation& obs, cv::Mat* overlay = nullptr);

    bool locked() const { return locked_; }
    void reset();

    // Last lock event ("locked on open hand", "released", ...) since the
    // previous call, for the log; empty if nothing happened.
    std::string take_event();

    // Tests only: use these face rectangles instead of running the cascades.
    void set_faces_for_test(const std::vector<cv::Rect>& faces);

    // Stateless measurement of the largest skin blob (no faces, no lock).
    // Kept for the unit tests of the openness measure.
    bool detect(const cv::Mat& frame, float& openness, float& confidence,
                cv::Mat* overlay = nullptr);

private:
    struct Blob {
        std::vector<cv::Point> contour;
        double area = 0;
        cv::Point2f center;
        cv::Rect box;
        float openness = 0;
        int gaps = 0;
        double solidity = 1.0;
        cv::Point2f palm;            // centre of the largest inscribed circle
        float palm_r = 0;            // its radius
        int fingers = 0;             // finger-wide strips crossing the ring
        float ring_r = 0;            // ring radius that gave `fingers`
        std::vector<cv::Point> finger_pts;
        std::vector<cv::Point> hull;
        std::vector<cv::Point> valleys;
    };

    cv::Mat skin_mask(const cv::Mat& frame) const;
    std::vector<Blob> blobs(const cv::Mat& mask, double frame_area) const;
    static void measure(Blob& b, double frame_diag);
    static void palm_and_fingers(Blob& b);
    bool is_open_hand(const Blob& b) const;
    bool is_fist(const Blob& b, double frame_area) const;
    void update_faces(const cv::Mat& frame, const cv::Mat& skin);
    // Fraction (0..1) of the blob's outline that runs along the face mask.
    double mask_contact(const Blob& b) const;
    void draw(cv::Mat& img, const Blob* hand) const;

    double min_area_ratio_;

    // faces
    cv::CascadeClassifier face_, profile_;
    bool face_ok_ = false, profile_ok_ = false;
    std::vector<cv::Rect> faces_;
    std::vector<cv::Rect> masked_;      // bounding boxes of the masked faces (labels)
    cv::Mat face_mask_;                 // face oval + neck, erased from the skin mask
    cv::Mat face_halo_;                 // the same, a few pixels bigger (contact test)
    int frames_since_face_ = 1000;
    int frames_face_held_ = 0;          // frames a face rect is kept without the cascade
    int frame_no_ = 0;
    bool manual_faces_ = false;

    // acquisition (needs several consecutive confirming frames)
    cv::Point2f pending_center_;
    int pending_frames_ = 0;
    bool pending_open_ = false;

    // lock
    bool locked_ = false;
    cv::Point2f last_center_;
    double last_area_ = 0;
    double area_open_ = 0, area_fist_ = 0;  // smoothed hand size per pose (merge detection)
    float lock_palm_r_ = 0;
    cv::Rect last_box_;
    int lost_frames_ = 0;
    std::string event_;
};
