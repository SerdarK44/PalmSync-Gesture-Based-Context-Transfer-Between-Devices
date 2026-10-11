// Deterministic tests for HandDetector using synthetic, skin-coloured images
// (no camera needed):
//  - the openness measure separates a fist from a spread hand;
//  - the LOCK-ON tracker never locks onto a face-like blob, locks onto an
//    open hand even when the face is bigger, stays on the hand while it
//    closes into a fist next to the face, and releases the lock when the
//    hand leaves.

// Tests are built in Release (WDAC blocks Debug exes) — keep assert() active.
#undef NDEBUG
#include "hand_detector.hpp"

#include <opencv2/imgproc.hpp>

#include <cassert>
#include <cmath>
#include <cstdio>

// A BGR value that lands inside the YCrCb skin range the detector uses.
static const cv::Scalar kSkin(110, 140, 200);

static cv::Mat blank() { return cv::Mat(480, 640, CV_8UC3, cv::Scalar(0, 0, 0)); }

// Big oval with no fingers: what the old detector mistook for a fist.
static void draw_face(cv::Mat& img) {
    cv::ellipse(img, {170, 210}, {105, 135}, 0, 0, 360, kSkin, cv::FILLED);
}

static void draw_open_hand(cv::Mat& img, cv::Point c) {
    cv::circle(img, c, 55, kSkin, cv::FILLED);  // palm
    for (double deg : {-160.0, -125.0, -90.0, -55.0, -20.0}) {
        double rad = deg * CV_PI / 180.0;
        cv::Point tip(c.x + static_cast<int>(140 * std::cos(rad)),
                      c.y + static_cast<int>(140 * std::sin(rad)));
        cv::line(img, c, tip, kSkin, 22);
    }
}

static void draw_fist(cv::Mat& img, cv::Point c) {
    cv::circle(img, c, 62, kSkin, cv::FILLED);
}

static const cv::Point kHand(470, 310);

void test_openness_measure() {
    HandDetector d;
    cv::Mat fist = blank(), open = blank();
    draw_fist(fist, {320, 240});
    draw_open_hand(open, {320, 260});

    float fo = -1, oo = -1, conf = 0;
    assert(d.detect(fist, fo, conf));
    assert(d.detect(open, oo, conf));
    std::printf("fist openness=%.2f  open openness=%.2f\n", fo, oo);
    assert(fo < 0.3f && "a solid blob reads as a fist");
    assert(oo > fo + 0.3f && "a spread hand reads clearly more open");
    std::printf("PASS: test_openness_measure\n");
}

void test_face_alone_never_locks() {
    HandDetector d;
    cv::Mat img = blank();
    draw_face(img);
    HandObservation obs;
    for (int i = 0; i < 60; i++) assert(!d.track(img, obs) && "a face must never become the hand");
    assert(!d.locked());
    std::printf("PASS: test_face_alone_never_locks\n");
}

void test_locks_hand_not_bigger_face_and_follows_it_into_a_fist() {
    HandDetector d;
    HandObservation obs;

    cv::Mat open = blank();
    draw_face(open);
    draw_open_hand(open, kHand);
    for (int i = 0; i < 8; i++) d.track(open, obs);
    assert(d.locked() && "locks after a few confirming frames");
    assert(d.track(open, obs));
    std::printf("open: fingers=%d gaps=%d openness=%.2f cx=%.2f\n", obs.fingers, obs.finger_gaps, obs.openness, obs.cx);
    assert(obs.fingers >= 4 && obs.finger_gaps >= 3);
    assert(obs.cx > 0.6f && "the lock is on the hand (right side), not the bigger face (left)");

    // Hand closes into a fist; the face is now by far the biggest blob.
    cv::Mat fist = blank();
    draw_face(fist);
    draw_fist(fist, kHand);
    for (int i = 0; i < 10; i++) assert(d.track(fist, obs));
    std::printf("fist: openness=%.2f cx=%.2f\n", obs.openness, obs.cx);
    assert(obs.cx > 0.6f && "the tag stays on the hand, it doesn't jump to the face");
    assert(obs.openness < 0.3f);
    std::printf("PASS: test_locks_hand_not_bigger_face_and_follows_it_into_a_fist\n");
}

void test_lock_released_when_hand_leaves() {
    HandDetector d;
    HandObservation obs;
    cv::Mat open = blank();
    draw_face(open);
    draw_open_hand(open, kHand);
    for (int i = 0; i < 8; i++) d.track(open, obs);
    assert(d.locked());

    cv::Mat face_only = blank();
    draw_face(face_only);
    for (int i = 0; i < 30; i++) assert(!d.track(face_only, obs) && "never falls back to the face");
    assert(!d.locked() && "lock released after the hand is gone");
    std::printf("PASS: test_lock_released_when_hand_leaves\n");
}

void test_fist_alone_needs_known_face_to_lock() {
    // Without a face model the detector can't tell a fist from a face, so a
    // fist alone must not acquire the lock.
    HandDetector d;
    HandObservation obs;
    cv::Mat fist = blank();
    draw_fist(fist, kHand);
    for (int i = 0; i < 10; i++) assert(!d.track(fist, obs));
    std::printf("PASS: test_fist_alone_needs_known_face_to_lock\n");
}

void test_face_remnants_beside_the_mask_never_lock() {
    // The bug seen with a real camera: the face box covers only the inner
    // face, the cheek/ear strips left over at its sides were locked onto as
    // a "hand". Here the face is much wider than its detected box.
    HandDetector d;
    d.set_faces_for_test({cv::Rect(100, 120, 140, 170)});
    HandObservation obs;

    cv::Mat face = blank();
    cv::ellipse(face, {170, 210}, {150, 150}, 0, 0, 360, kSkin, cv::FILLED);
    for (int i = 0; i < 60; i++)
        assert(!d.track(face, obs) && "leftover face strips must never become the hand");
    assert(!d.locked());

    // A real fist away from the face can lock (the face is known)...
    cv::Mat with_fist = face.clone();
    draw_fist(with_fist, kHand);
    for (int i = 0; i < 8; i++) d.track(with_fist, obs);
    assert(d.locked() && d.track(with_fist, obs));
    assert(obs.cx > 0.6f && obs.openness < 0.3f);

    // ...and opening it is followed (this is the DROP path: fist -> open).
    cv::Mat with_open = face.clone();
    draw_open_hand(with_open, kHand);
    for (int i = 0; i < 5; i++) assert(d.track(with_open, obs));
    assert(obs.cx > 0.6f && obs.fingers >= 3);
    std::printf("PASS: test_face_remnants_beside_the_mask_never_lock\n");
}

void test_jagged_unmasked_face_never_locks() {
    // Face NOT found by the cascade (head turned / bad light), and hair,
    // glasses or a collar cut notches into the skin blob that look like
    // finger valleys. Convexity gaps alone used to accept this as an open
    // hand; the palm-circle ring finds no finger-wide strips around it.
    HandDetector d;
    cv::Mat img = blank();
    cv::ellipse(img, {300, 230}, {120, 150}, 0, 0, 360, kSkin, cv::FILLED);
    for (int x : {230, 270, 330, 370})            // hair strands from the top
        cv::line(img, {x, 70}, {x + 10, 170}, cv::Scalar(0, 0, 0), 9);
    cv::line(img, {200, 230}, {260, 215}, cv::Scalar(0, 0, 0), 7);   // glasses
    cv::line(img, {340, 215}, {400, 230}, cv::Scalar(0, 0, 0), 7);

    HandObservation obs;
    for (int i = 0; i < 60; i++) {
        bool got = d.track(img, obs);
        if (got) std::fprintf(stderr, "  jagged face locked: fingers=%d gaps=%d openness=%.2f\n", obs.fingers, obs.finger_gaps, obs.openness);
        assert(!got && "a jagged face is not an open hand");
    }
    assert(!d.locked());
    std::printf("PASS: test_jagged_unmasked_face_never_locks\n");
}

// Hand moved right next to the face (fingers reach the face's edge).
static const cv::Point kHandBesideFace(385, 300);

void test_hand_beside_masked_face_keeps_tag() {
    // Face known and masked: the hand next to it is cut by the mask edge,
    // but it's still the same hand — the tag must stay on it.
    HandDetector d;
    d.set_faces_for_test({cv::Rect(95, 110, 150, 190)});
    HandObservation obs;

    cv::Mat apart = blank();
    draw_face(apart);
    draw_open_hand(apart, kHand);
    for (int i = 0; i < 8; i++) d.track(apart, obs);
    assert(d.locked());

    cv::Mat beside = blank();
    draw_face(beside);
    draw_open_hand(beside, kHandBesideFace);
    for (int i = 0; i < 10; i++) {
        bool got = d.track(beside, obs);
        assert(got && "hand beside the face is still followed");
        assert(obs.cx > 0.5f && "tag stays on the hand, not on the face");
    }
    std::printf("PASS: test_hand_beside_masked_face_keeps_tag\n");
}

void test_hand_merging_with_unmasked_face_never_moves_tag_to_face() {
    // Face NOT known (cascade missed it): hand and face skin merge into one
    // big blob. That blob must not be taken as the hand.
    HandDetector d;
    HandObservation obs;

    cv::Mat apart = blank();
    draw_face(apart);
    draw_open_hand(apart, kHand);
    for (int i = 0; i < 8; i++) d.track(apart, obs);
    assert(d.locked());

    cv::Mat merged = blank();
    draw_face(merged);
    draw_open_hand(merged, kHandBesideFace);
    for (int i = 0; i < 30; i++) {
        bool got = d.track(merged, obs);
        if (got) std::fprintf(stderr, "  merged blob taken: cx=%.2f\n", obs.cx);
        assert(!got && "a hand+face blob is not the hand");
    }
    assert(!d.locked() && "lock released instead of sliding onto the face");
    std::printf("PASS: test_hand_merging_with_unmasked_face_never_moves_tag_to_face\n");
}

void test_hand_in_corner_beside_chin_locks() {
    // The left-hand problem: face slightly off-centre, hand held low beside
    // the chin with the fingers pointing towards the face. The old
    // RECTANGULAR mask covered that corner, cut the fingers off and rejected
    // the hand as "touching the face". The oval mask leaves it free.
    HandDetector d;
    d.set_faces_for_test({cv::Rect(250, 100, 140, 170)});
    HandObservation obs;

    cv::Mat img = blank();
    cv::ellipse(img, {320, 176}, {85, 130}, 0, 0, 360, kSkin, cv::FILLED);   // face
    draw_open_hand(img, {130, 365});                                         // hand low-left
    for (int i = 0; i < 8; i++) d.track(img, obs);
    assert(d.locked() && "hand beside the chin locks");
    assert(d.track(img, obs) && obs.fingers >= 4 && obs.cx < 0.35f);
    std::printf("corner hand: fingers=%d gaps=%d openness=%.2f\n", obs.fingers, obs.finger_gaps, obs.openness);
    std::printf("PASS: test_hand_in_corner_beside_chin_locks\n");
}

int main() {
    test_openness_measure();
    test_face_alone_never_locks();
    test_locks_hand_not_bigger_face_and_follows_it_into_a_fist();
    test_lock_released_when_hand_leaves();
    test_fist_alone_needs_known_face_to_lock();
    test_face_remnants_beside_the_mask_never_lock();
    test_jagged_unmasked_face_never_locks();
    test_hand_beside_masked_face_keeps_tag();
    test_hand_merging_with_unmasked_face_never_moves_tag_to_face();
    test_hand_in_corner_beside_chin_locks();
    std::printf("\nAll detector tests passed. (10/10)\n");
    return 0;
}
