# 👁️ Faz 2: Görüntü İşleme ve El Takibi (The Eye)

**Süre**: 3-4 hafta | **Öncelik**: ⭐⭐⭐ (Kritik) | **Bağımlılık**: Faz 1 (Networking)

---

## 📍 Hedef

PalmSync'in "görüş sistemi" olacak C++ servisini geliştirecek. Standart web kamerası vasıtasıyla el hareketlerini (GRAB/DROP) gerçek zamanlı takip edecek, jest durum makinesini çalıştıracak ve Rust ajanına UDP IPC üzerinden sinyaller göndereceğiz.

---

## ✅ Başarı Kriterleri

- [ ] OpenCV ve MediaPipe C++ bindings'i entegre edilir
- [ ] 30 FPS'de webcam stream işlenir
- [ ] El üzerinde 21 landmark noktası detekt edilir
- [ ] Jest state machine (GRAB/DROP) doğru çalışır
- [ ] Yanlış tetiklemeler debounce edilerek filtrelenir
- [ ] UDP socket'i açılır ve `localhost:12345`'de dinler
- [ ] Jest event'leri JSON formatında Rust ajanına gönderilir
- [ ] Servis gizli modda (no console window) çalışır
- [ ] Kamera erişimi kontrollü şekilde yönetilir (aktivasyon/deaktivasyondan)

---

## 🎯 Görevler (Checklist)

### 1. C++ Proje Yapısı ve Build System

```
eye/
├── CMakeLists.txt              # CMake konfigurasyonu
├── src/
│   ├── main.cpp               # Entry point
│   ├── camera_manager.hpp/cpp # Kamera işlemleri
│   ├── hand_detector.hpp/cpp  # MediaPipe entegrasyonu
│   ├── gesture_fsm.hpp/cpp    # GRAB/DROP state machine
│   ├── ipc_sender.hpp/cpp     # UDP socket yönetimi
│   └── utils.hpp/cpp          # Yardımcı fonksiyonlar
├── include/
│   └── palmsync_eye.h         # Public header
├── vcpkg.json                 # Bağımlılık yönetimi (opsiyonel)
└── tests/
    └── gesture_test.cpp       # Unit testler
```

**Yapılacaklar:**
- [ ] CMakeLists.txt oluştur (Visual Studio 2022 compatible)
- [ ] vcpkg veya Conan kullanarak OpenCV ve MediaPipe bağımlılıklarını kur
- [ ] Compiler flags'ı set et (`/W4`, `/O2`, `/std:c++17`)
- [ ] Debug ve Release build configuration'ını kur

**`CMakeLists.txt` örneği:**

```cmake
cmake_minimum_required(VERSION 3.20)
project(palmsync-eye VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Find packages
find_package(OpenCV REQUIRED)
find_package(mediapipe REQUIRED)

# Main executable
add_executable(palmsync-eye
    src/main.cpp
    src/camera_manager.cpp
    src/hand_detector.cpp
    src/gesture_fsm.cpp
    src/ipc_sender.cpp
    src/utils.cpp
)

target_link_libraries(palmsync-eye
    opencv::opencv_core
    opencv::opencv_highgui
    opencv::opencv_videoio
    mediapipe::mediapipe
)

target_include_directories(palmsync-eye PRIVATE include)
```

---

### 2. Kamera Yönetimi

**`src/camera_manager.hpp`:**

```cpp
#pragma once
#include <opencv2/videoio.hpp>
#include <memory>

class CameraManager {
public:
    CameraManager(int camera_id = 0);
    ~CameraManager();
    
    bool initialize();
    bool capture_frame(cv::Mat& frame);
    void release();
    bool is_available() const;
    void set_fps(int fps);
    
private:
    cv::VideoCapture cap_;
    int camera_id_;
    bool initialized_;
};
```

**Yapılacaklar:**
- [ ] OpenCV VideoCapture ile kamera başlat
- [ ] Default resolution: 640x480
- [ ] FPS: 30 (cap_.set(cv::CAP_PROP_FPS, 30))
- [ ] Error handling (kamera bulunamadığında)
- [ ] Resource cleanup (destructor'da release())

---

### 3. MediaPipe Hand Detection

**`src/hand_detector.hpp`:**

```cpp
#pragma once
#include <mediapipe/framework/calculator_graph.h>
#include <mediapipe/framework/formats/landmark.pb.h>
#include <opencv2/core.hpp>
#include <vector>

struct HandLandmark {
    float x, y, z;
    float confidence;
};

class HandDetector {
public:
    HandDetector();
    ~HandDetector();
    
    bool initialize();
    bool detect(const cv::Mat& frame, std::vector<HandLandmark>& landmarks);
    void shutdown();
    
private:
    std::unique_ptr<mediapipe::CalculatorGraph> graph_;
    bool initialized_;
};
```

**Yapılacaklar:**
- [ ] MediaPipe CalculatorGraph'i kur
- [ ] Hand detection model'i yükle
- [ ] Frame'i normalize et (0-1 range)
- [ ] 21 landmark noktası extract et
- [ ] Confidence score'u al (filtreleme için)
- [ ] GPU acceleration'ı enable et (optional)

**MediaPipe Graph Configuration:**

```pbtxt
# hand_tracking_desktop.pbtxt
input_stream: "input_video"
output_stream: "output_landmarks"

node {
  calculator: "MediaPipeHand"
  input_stream: "VIDEO:input_video"
  output_stream: "LANDMARKS:output_landmarks"
}
```

---

### 4. Jest Durum Makinesi (Gesture FSM)

**`src/gesture_fsm.hpp`:**

```cpp
#pragma once
#include "hand_detector.hpp"
#include <chrono>
#include <functional>

enum class GestureEvent {
    GRAB,      // El kapandı
    DROP,      // El açıldı
    NEUTRAL    // Durağan
};

class GestureFSM {
public:
    using EventCallback = std::function<void(GestureEvent)>;
    
    GestureFSM(float close_threshold = 0.3, int debounce_ms = 200);
    
    void update(const std::vector<HandLandmark>& landmarks);
    void set_event_callback(EventCallback callback);
    
private:
    enum class State {
        OPEN,
        CLOSING,
        CLOSED,
        OPENING
    };
    
    State current_state_ = State::OPEN;
    std::chrono::time_point<std::chrono::high_resolution_clock> last_event_time_;
    int debounce_ms_;
    float close_threshold_;
    EventCallback callback_;
    
    float compute_hand_closure(const std::vector<HandLandmark>& landmarks);
    void emit_event(GestureEvent event);
};
```

**Pseudocode Algoritması:**

```
compute_hand_closure(landmarks):
    palm_center = (landmarks[0].x + landmarks[5].x + landmarks[17].x) / 3
    
    distances = []
    for finger_tip in [4, 8, 12, 16, 20]:  // Parmak uçları
        distance = euclidean_distance(landmarks[finger_tip], palm_center)
        distances.append(distance)
    
    closure = mean(distances)  // Ortalama mesafe
    return closure

update(landmarks):
    if landmarks.size() < 21:
        return  // Tam el görünmüyor
    
    closure = compute_hand_closure(landmarks)
    now = current_time()
    
    switch current_state:
        case OPEN:
            if closure < close_threshold:
                current_state = CLOSING
                
        case CLOSING:
            if closure >= close_threshold:
                current_state = OPEN
            else if (now - last_event_time) > debounce_ms:
                emit_event(GRAB)
                current_state = CLOSED
                last_event_time = now
        
        case CLOSED:
            if closure >= close_threshold:
                current_state = OPENING
                
        case OPENING:
            if closure < close_threshold:
                current_state = CLOSED
            else if (now - last_event_time) > debounce_ms:
                emit_event(DROP)
                current_state = OPEN
                last_event_time = now
```

**Yapılacaklar:**
- [ ] Palm center'ı hesapla
- [ ] Parmak uçlarından merkeze mesafe hesapla
- [ ] Closure ratio'yu (0-1) norm et
- [ ] Debounce logic'i implement et (yanlış tetiklemeler için)
- [ ] State transition'ları doğru yap

---

### 5. UDP IPC Sender

**`src/ipc_sender.hpp`:**

```cpp
#pragma once
#include "gesture_fsm.hpp"
#include <string>

struct GestureEvent {
    enum Type { GRAB, DROP } type;
    long long timestamp_ms;
};

class IPCSender {
public:
    IPCSender(const std::string& dest_addr = "127.0.0.1", int dest_port = 12345);
    ~IPCSender();
    
    bool initialize();
    bool send_event(GestureEvent event);
    void shutdown();
    
private:
    std::string dest_addr_;
    int dest_port_;
    int socket_;
    bool initialized_;
};
```

**Yapılacaklar:**
- [ ] UDP socket'i aç (WSASocket)
- [ ] Destination: localhost:12345
- [ ] Reconnection logic (socket fail ise retry)
- [ ] JSON event format: `{"type":"GRAB","timestamp_ms":1234567890}`
- [ ] Non-blocking send
- [ ] Error logging

**JSON Event Format:**

```json
{
    "event": "GRAB",
    "timestamp_ms": 1689678912345,
    "gesture_confidence": 0.98
}
```

---

### 6. Main Event Loop

**`src/main.cpp`:**

```cpp
#include "camera_manager.hpp"
#include "hand_detector.hpp"
#include "gesture_fsm.hpp"
#include "ipc_sender.hpp"
#include <iostream>

int main() {
    // Initialize components
    CameraManager camera(0);
    HandDetector detector;
    GestureFSM fsm(0.3f, 200);
    IPCSender sender("127.0.0.1", 12345);
    
    if (!camera.initialize()) {
        std::cerr << "Failed to initialize camera\n";
        return 1;
    }
    
    if (!detector.initialize()) {
        std::cerr << "Failed to initialize detector\n";
        return 1;
    }
    
    if (!sender.initialize()) {
        std::cerr << "Failed to initialize IPC sender\n";
        return 1;
    }
    
    // Set gesture callback
    fsm.set_event_callback([&sender](GestureEvent event) {
        GestureEvent evt{event, std::chrono::high_resolution_clock::now().time_since_epoch().count()};
        sender.send_event(evt);
    });
    
    // Main loop (30 FPS)
    cv::Mat frame;
    while (true) {
        if (!camera.capture_frame(frame)) {
            continue;
        }
        
        std::vector<HandLandmark> landmarks;
        if (detector.detect(frame, landmarks)) {
            fsm.update(landmarks);
        }
        
        // ~33ms per frame (30 FPS)
        std::this_thread::sleep_for(std::chrono::milliseconds(33));
    }
    
    camera.release();
    detector.shutdown();
    sender.shutdown();
    
    return 0;
}
```

**Yapılacaklar:**
- [ ] Capture loop'u kur (30 FPS)
- [ ] Frame processing pipeline'ı başlat
- [ ] Error handling'i implement et
- [ ] Graceful shutdown'u implement et (Ctrl+C)
- [ ] Performance monitoring (add logging)

---

### 7. Windows Daemonization

**`src/main.cpp` başlangıcında:**

```cpp
#ifdef _WIN32
#include <windows.h>

// Windows subsystem olarak derlenmesi için:
// #pragma comment(linker, "/SUBSYSTEM:WINDOWS /ENTRY:mainCRTStartup")

// Console penceresini gizle
int main() {
    #ifdef _WIN32
    HWND hwnd = GetConsoleWindow();
    ShowWindow(hwnd, SW_HIDE);
    #endif
    
    // ... rest of code
}
#endif
```

**CMakeLists.txt'ye ekle:**

```cmake
if(WIN32)
    set_target_properties(palmsync-eye PROPERTIES
        WIN32_EXECUTABLE ON
    )
endif()
```

**Yapılacaklar:**
- [ ] Console window'u hide et
- [ ] Windows Service registration'ı hazırla (Faz 4'de)
- [ ] Event logging'i implement et

---

### 8. Test Suite

**`tests/gesture_test.cpp`:**

```cpp
#include <gtest/gtest.h>
#include "gesture_fsm.hpp"

class GestureTest : public ::testing::Test {
protected:
    GestureFSM fsm{0.3f, 200};
    std::vector<GestureEvent> captured_events;
    
    void SetUp() override {
        fsm.set_event_callback([this](GestureEvent event) {
            captured_events.push_back(event);
        });
    }
};

TEST_F(GestureTest, GrabEventTriggered) {
    // Create landmarks where fingers are close to palm
    std::vector<HandLandmark> landmarks(21);
    for (int i = 0; i < 21; i++) {
        landmarks[i] = {0.5f, 0.5f, 0.5f, 0.99f};  // All close together
    }
    
    fsm.update(landmarks);
    std::this_thread::sleep_for(std::chrono::milliseconds(250));  // Wait for debounce
    
    ASSERT_EQ(captured_events.size(), 1);
    ASSERT_EQ(captured_events[0], GestureEvent::GRAB);
}

TEST_F(GestureTest, DebounceWorks) {
    // Rapid updates should not produce multiple events
    std::vector<HandLandmark> landmarks(21);
    for (int i = 0; i < 21; i++) {
        landmarks[i] = {0.5f, 0.5f, 0.5f, 0.99f};
    }
    
    fsm.update(landmarks);
    fsm.update(landmarks);
    fsm.update(landmarks);
    
    std::this_thread::sleep_for(std::chrono::milliseconds(250));
    
    // Should only have 1 event despite 3 updates
    ASSERT_LE(captured_events.size(), 1);
}
```

**Yapılacaklar:**
- [ ] Google Test framework'ü integrate et
- [ ] Gesture detection testleri yaz
- [ ] Debounce logic testleri yaz
- [ ] Camera initialization testleri yaz
- [ ] UDP sender testleri yaz

---

### 9. Performans Optimizasyonu

**Yapılacaklar:**
- [ ] Frame processing'i CPU profile et
- [ ] GPU acceleration ekle (CUDA veya Metal)
- [ ] Memory leak check'i yapı (valgrind/Dr. Memory)
- [ ] CPU usage'ı < %15'e tut
- [ ] Latency measurement'ı implement et (capture -> event: < 100ms)

---

### 10. Logging ve Debugging

**`src/utils.cpp`:**

```cpp
#include <fstream>
#include <chrono>
#include <iostream>

void log_event(const std::string& message) {
    auto now = std::chrono::system_clock::now();
    auto time = std::chrono::system_clock::to_time_t(now);
    
    std::ofstream log_file("palmsync-eye.log", std::ios::app);
    log_file << "[" << std::ctime(&time) << "] " << message << "\n";
    log_file.close();
    
    #ifdef DEBUG
    std::cerr << message << "\n";
    #endif
}
```

**Yapılacaklar:**
- [ ] File-based logging implement et
- [ ] Console logging (debug mode'da)
- [ ] Event timestamp'lerini log et
- [ ] Performance metrics'i log et
- [ ] Error log'ları yazı

---

## 🧪 Doğrulama ve QA

```bash
# Build et
cd eye
mkdir build && cd build
cmake ..
cmake --build . --config Release

# Tests çalıştır
ctest -VV

# Performance test et
palmsync-eye.exe --benchmark

# Logs kontrol et
type palmsync-eye.log
```

---

## 📦 Deliverables

Bu faz tamamlandığında:

1. ✅ **`palmsync-eye.exe`**: Çalışan kamera servis
2. ✅ **Jest Detection**: GRAB/DROP events
3. ✅ **IPC Communication**: UDP JSON messages
4. ✅ **Test Suite**: %80+ code coverage
5. ✅ **Logging**: File-based event logs
6. ✅ **Performance**: <15% CPU, <100ms latency

---

## 🔄 Sonraki Faz Bağlantıları

- **Faz 3 (The Brain)**: UDP listener'ı (localhost:12345) implement edecek
- **Faz 4 (Deployment)**: eye.exe'ı service olarak register edecek

---

## 📝 Bağımlılıklar ve Uyarılar

⚠️ **Önemli Noktalar:**
- OpenCV ve MediaPipe C++ bindings'i compile etmek zaman alabilir (1-2 saat)
- Webcam permissions'ı Windows'da check edin
- GPU driver'ları güncel tutun (CUDA için)
- Test sırasında iyi aydınlatmaya dikkat edin
- Çoklu el scenario'sunda sadece dominant hand takip et

🔐 **Güvenlik:**
- Kamera erişimini Rust ajanından enable/disable et (power management)
- Sensitive gesture data'sını UDP'de plaintext olarak yapmayın (sonra encrypt)

---

**Son Güncelleme**: 2026-07-20  
**Statüsü**: 🔵 Hazır Implementation için
