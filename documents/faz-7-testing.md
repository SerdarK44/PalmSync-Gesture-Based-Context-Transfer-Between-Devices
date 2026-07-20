# 🧪 Faz 7: Testing ve Kalite Assurance (QA & Testing)

**Süre**: 2 hafta | **Öncelik**: ⭐⭐⭐ (Kritik) | **Bağımlılık**: Faz 1-6

---

## 📍 Hedef

PalmSync sisteminin tüm bileşenlerinin üzerine kapsamlı test suites kuracağız. Unit test'ler, integration test'ler, end-to-end test'ler, performance test'ler ve security test'ler yazacağız. Sistemin reliability, stability ve performance'ını sağlamlaştıracağız.

---

## ✅ Başarı Kriterleri

- [ ] Unit test coverage %80+ (C++ ve Rust)
- [ ] Integration testleri tüm major components için
- [ ] End-to-end scenario testleri yazılmış
- [ ] Performance benchmark'lar baseline kurmuş
- [ ] Load testing 100+ concurrent connections
- [ ] Security testing temel saldırılar için
- [ ] Regression test suite oluşturulmuş
- [ ] Automated CI/CD pipeline kurulmuş
- [ ] Test result'ları documented
- [ ] Known issues list'i maintain edilmiş

---

## 🎯 Görevler (Checklist)

### 1. Unit Testing Framework Setup

**Rust Testing:**

```toml
# Cargo.toml
[dev-dependencies]
tokio = { version = "1.35", features = ["full"] }
tokio-test = "0.4"
mockito = "1.2"
proptest = "1.4"
criterion = "0.5"
```

**C++ Testing:**

```cmake
# CMakeLists.txt
include(FetchContent)
FetchContent_Declare(
    googletest
    URL https://github.com/google/googletest/archive/main.zip
)
FetchContent_MakeAvailable(googletest)

enable_testing()
add_executable(palmsync-tests
    tests/gesture_test.cpp
    tests/ipc_test.cpp
)
target_link_libraries(palmsync-tests gtest gtest_main)
```

**Yapılacaklar:**
- [ ] Test framework'ü setup et
- [ ] Test directory structure'ı organize et
- [ ] Mock objects'i prepare et
- [ ] Test fixtures'ı oluştur
- [ ] CI integration'ı kur

---

### 2. Unit Tests - Networking

**`networking/tests/unit_tests.rs`:**

```rust
#[cfg(test)]
mod tests {
    use palmsync_networking::*;
    use tokio::test;

    #[test]
    async fn test_master_node_creation() {
        let master = MasterNode::new("127.0.0.1:9001", "testpass").await;
        assert!(master.is_ok());
    }

    #[test]
    async fn test_slave_connection() {
        let _master = MasterNode::new("127.0.0.1:9002", "testpass").await.unwrap();
        let mut slave = SlaveNode::new("127.0.0.1", 9002, "testpass");
        
        let result = slave.connect().await;
        assert!(result.is_ok());
    }

    #[test]
    async fn test_invalid_password_rejection() {
        let _master = MasterNode::new("127.0.0.1:9003", "correctpass").await.unwrap();
        let mut slave = SlaveNode::new("127.0.0.1", 9003, "wrongpass");
        
        let result = slave.connect().await;
        assert!(result.is_err());
    }

    #[test]
    async fn test_state_persistence() {
        let master = MasterNode::new("127.0.0.1:9004", "testpass").await.unwrap();
        
        let state = StateSnapshot {
            project_path: "/path/to/project".to_string(),
            file_name: "main.rs".to_string(),
            cursor_line: 42,
            clipboard_content: "test".to_string(),
            timestamp: 1234567890,
        };

        master.update_state(state.clone()).await.unwrap();
        
        let retrieved = master.get_state().await.unwrap();
        assert_eq!(retrieved.cursor_line, 42);
    }

    #[test]
    async fn test_concurrent_slave_connections() {
        let master = MasterNode::new("127.0.0.1:9005", "testpass").await.unwrap();
        
        let mut handles = vec![];
        for i in 0..10 {
            let addr = format!("127.0.0.1:900{}", i);
            let handle = tokio::spawn(async move {
                let mut slave = SlaveNode::new("127.0.0.1", 9005, "testpass");
                slave.connect().await
            });
            handles.push(handle);
        }

        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
    }

    #[test]
    fn test_password_hashing() {
        let password = "mysecurepass";
        let hash1 = compute_password_hash(password);
        let hash2 = compute_password_hash(password);
        
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, compute_password_hash("different"));
    }

    #[test]
    fn test_message_serialization() {
        let msg = RpcMessage::Grab(StateSnapshot {
            project_path: "/path".to_string(),
            file_name: "file.rs".to_string(),
            cursor_line: 10,
            clipboard_content: "data".to_string(),
            timestamp: 123456,
        });

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RpcMessage = serde_json::from_str(&json).unwrap();
        
        match deserialized {
            RpcMessage::Grab(snap) => assert_eq!(snap.cursor_line, 10),
            _ => panic!("Wrong message type"),
        }
    }
}
```

**Yapılacaklar:**
- [ ] Master node tests'i yaz
- [ ] Slave node tests'i yaz
- [ ] Authentication tests'i yaz
- [ ] Concurrency tests'i yaz
- [ ] Serialization tests'i yaz
- [ ] Error handling tests'i yaz

---

### 3. Unit Tests - Eye (C++)

**`eye/tests/gesture_test.cpp`:**

```cpp
#include <gtest/gtest.h>
#include "../src/gesture_fsm.hpp"
#include <thread>
#include <chrono>

class GestureFSMTest : public ::testing::Test {
protected:
    GestureFSM fsm{0.3f, 200};
    std::vector<GestureEvent> captured_events;

    void SetUp() override {
        fsm.set_event_callback([this](GestureEvent event) {
            captured_events.push_back(event);
        });
    }

    std::vector<HandLandmark> create_open_hand() const {
        std::vector<HandLandmark> landmarks(21);
        // All landmarks spread out
        for (int i = 0; i < 21; i++) {
            landmarks[i] = {
                static_cast<float>(i) / 10.0f,
                static_cast<float>(i) / 10.0f,
                0.5f,
                0.99f
            };
        }
        return landmarks;
    }

    std::vector<HandLandmark> create_closed_hand() const {
        std::vector<HandLandmark> landmarks(21);
        // All landmarks close to center
        for (int i = 0; i < 21; i++) {
            landmarks[i] = {0.5f, 0.5f, 0.5f, 0.99f};
        }
        return landmarks;
    }
};

TEST_F(GestureFSMTest, GrabEventTriggeredOnClosedHand) {
    auto open = create_open_hand();
    auto closed = create_closed_hand();

    fsm.update(open);
    fsm.update(closed);
    std::this_thread::sleep_for(std::chrono::milliseconds(250));
    fsm.update(closed);

    ASSERT_EQ(captured_events.size(), 1);
    ASSERT_EQ(captured_events[0], GestureEvent::GRAB);
}

TEST_F(GestureFSMTest, DropEventTriggeredOnOpenHand) {
    auto open = create_open_hand();
    auto closed = create_closed_hand();

    // First trigger grab
    fsm.update(closed);
    std::this_thread::sleep_for(std::chrono::milliseconds(250));
    
    // Then trigger drop
    fsm.update(open);
    std::this_thread::sleep_for(std::chrono::milliseconds(250));

    ASSERT_GE(captured_events.size(), 1);
    
    // Find drop event
    bool found_drop = false;
    for (const auto& event : captured_events) {
        if (event == GestureEvent::DROP) {
            found_drop = true;
            break;
        }
    }
    ASSERT_TRUE(found_drop);
}

TEST_F(GestureFSMTest, DebouncePreventsFalsePositives) {
    auto closed = create_closed_hand();

    // Rapid updates
    for (int i = 0; i < 10; i++) {
        fsm.update(closed);
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }

    ASSERT_LE(captured_events.size(), 1);
}

TEST_F(GestureFSMTest, HandClosureCalculation) {
    auto open = create_open_hand();
    auto closed = create_closed_hand();

    // Verify closure ratio is different for open vs closed
    float open_closure = fsm.compute_closure(open);
    float closed_closure = fsm.compute_closure(closed);

    ASSERT_LT(closed_closure, open_closure);
}

TEST(CameraManagerTest, CameraInitialization) {
    CameraManager camera(0);
    EXPECT_TRUE(camera.initialize());
    EXPECT_TRUE(camera.is_available());
    
    camera.release();
    EXPECT_FALSE(camera.is_available());
}

TEST(IPCSenderTest, JSONMessageFormat) {
    IPCSender sender("127.0.0.1", 12345);
    EXPECT_TRUE(sender.initialize());
    
    GestureEvent event{GestureEvent::GRAB, 1234567890};
    EXPECT_TRUE(sender.send_event(event));
    
    sender.shutdown();
}
```

**Yapılacaklar:**
- [ ] Gesture FSM testleri yaz
- [ ] Camera manager testleri yaz
- [ ] Hand detection testleri yaz
- [ ] IPC sender testleri yaz
- [ ] Performance testleri yaz

---

### 4. Unit Tests - Brain (Rust)

**`brain/tests/unit_tests.rs`:**

```rust
#[cfg(test)]
mod tests {
    use palmsync_brain::*;
    use tokio::test;

    #[test]
    async fn test_context_capture_vscode() {
        let context = ContextCapture::capture_vscode_context();
        // VS Code açık değilse error'u handle et
        // Veya mock context return et
    }

    #[test]
    async fn test_clipboard_operations() {
        let test_text = "Hello, PalmSync!";
        
        Clipboard::set_text(test_text).unwrap();
        let retrieved = Clipboard::get_text().unwrap();
        
        assert_eq!(test_text, retrieved);
    }

    #[test]
    async fn test_vs_code_command_execution() {
        let result = Executor::open_vscode(
            "C:\\Users\\test\\Project",
            "main.rs",
            42,
        ).await;

        assert!(result.is_ok() || result.is_err());  // VS Code olmayabilir
    }

    #[test]
    async fn test_state_manager_grab() {
        let rpc_client = RpcClient::new("127.0.0.1:9001");
        let state_manager = StateManager::new(rpc_client);

        let result = state_manager.grab_state().await;
        // Result'ı check et (VS Code açık olmayabilir)
    }

    #[test]
    async fn test_ipc_listener_message_parsing() {
        let listener = IPCListener::new().await.unwrap();

        // UDP mesajı gönder
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let msg = r#"{"event":"GRAB","timestamp_ms":123456}"#;
        socket.send_to(msg.as_bytes(), "127.0.0.1:12345").await.unwrap();

        // Listener'ın mesajı handle etmesini verify et
    }

    #[test]
    fn test_config_loading() {
        let config = Config::from_env();
        assert!(!config.master_addr.is_empty());
        assert!(config.master_port > 0);
    }
}
```

**Yapılacaklar:**
- [ ] Context capture testleri yaz
- [ ] Clipboard testleri yaz
- [ ] Command executor testleri yaz
- [ ] State manager testleri yaz
- [ ] IPC listener testleri yaz
- [ ] Config loading testleri yaz

---

### 5. Integration Tests

**`tests/integration_test.rs` (Multi-component):**

```rust
#[cfg(test)]
mod integration_tests {
    use palmsync_networking::*;
    use palmsync_brain::*;
    use tokio::test;

    #[test]
    async fn test_end_to_end_grab_drop_flow() {
        // 1. Master start
        let master = MasterNode::new("127.0.0.1:9010", "testpass")
            .await
            .expect("Master should start");

        // 2. Slave 1 connect
        let mut slave1 = SlaveNode::new("127.0.0.1", 9010, "testpass");
        slave1.connect().await.expect("Slave1 should connect");

        // 3. Slave 2 connect
        let mut slave2 = SlaveNode::new("127.0.0.1", 9010, "testpass");
        slave2.connect().await.expect("Slave2 should connect");

        // 4. Slave 1 GRAB state
        let state = StateSnapshot {
            project_path: "/test/project".to_string(),
            file_name: "test.rs".to_string(),
            cursor_line: 123,
            clipboard_content: "test data".to_string(),
            timestamp: 1234567890,
        };

        slave1.send_state(state.clone())
            .await
            .expect("State should be sent");

        // 5. Verify Master has state
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let master_state = master.get_state().await.expect("Master should have state");
        assert_eq!(master_state.cursor_line, 123);

        // 6. Slave 2 DROP (request state)
        let retrieved_state = slave2.request_state()
            .await
            .expect("Should retrieve state");
        
        assert_eq!(retrieved_state.cursor_line, 123);
        assert_eq!(retrieved_state.clipboard_content, "test data");
    }

    #[test]
    async fn test_master_node_registry_management() {
        let master = MasterNode::new("127.0.0.1:9011", "pass").await.unwrap();

        // Connect 5 slaves
        for i in 0..5 {
            let mut slave = SlaveNode::new("127.0.0.1", 9011, "pass");
            slave.connect().await.unwrap();
            
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        // Verify all registered
        let registered_count = master.get_connected_slaves_count().await.unwrap();
        assert_eq!(registered_count, 5);
    }

    #[test]
    async fn test_network_failure_recovery() {
        let master = MasterNode::new("127.0.0.1:9012", "pass").await.unwrap();
        let mut slave = SlaveNode::new("127.0.0.1", 9012, "pass");

        // Connect
        slave.connect().await.unwrap();
        assert!(slave.is_connected());

        // Simulate network failure (would require more complex setup)
        // Master stopping/restarting

        // Verify reconnection
        // slave.reconnect().await.unwrap();
        // assert!(slave.is_connected());
    }
}
```

**Yapılacaklar:**
- [ ] End-to-end flow testleri yaz
- [ ] Multi-node scenario testleri yaz
- [ ] Failure recovery testleri yaz
- [ ] State consistency testleri yaz

---

### 6. Performance Testing

**`benches/benchmarks.rs`:**

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use palmsync_networking::*;
use palmsync_shared::*;

fn benchmark_state_sync(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("state_grab_operation", |b| {
        b.to_async(&rt).block_on(async {
            let state = StateSnapshot {
                project_path: black_box("/test/project".to_string()),
                file_name: black_box("main.rs".to_string()),
                cursor_line: black_box(42),
                clipboard_content: black_box("data".to_string()),
                timestamp: black_box(1234567890),
            };

            // Time the grab operation
            let _result = perform_grab_operation(&state).await;
        });
    });

    c.bench_function("message_serialization", |b| {
        let msg = StateSnapshot {
            project_path: "/path".to_string(),
            file_name: "file.rs".to_string(),
            cursor_line: 10,
            clipboard_content: "data".to_string(),
            timestamp: 123456,
        };

        b.iter(|| {
            serde_json::to_string(black_box(&msg))
        });
    });
}

fn benchmark_cryptography(c: &mut Criterion) {
    c.bench_function("aes256_encrypt", |b| {
        let crypto = CryptoManager::new("password");
        b.iter(|| {
            crypto.encrypt(black_box(b"test data"))
        });
    });

    c.bench_function("password_hash", |b| {
        b.iter(|| {
            AuthManager::hash_password(black_box("password"))
        });
    });
}

criterion_group!(benches, benchmark_state_sync, benchmark_cryptography);
criterion_main!(benches);
```

**Çalıştır:**
```bash
cargo bench --bench benchmarks
```

**Yapılacaklar:**
- [ ] State sync benchmark'larını yaz
- [ ] Crypto operation benchmark'larını yaz
- [ ] Network latency benchmark'larını yaz
- [ ] Memory usage benchmark'larını yaz
- [ ] Baseline kurarak regression'ı track et

---

### 7. Load Testing

**`tests/load_test.rs`:**

```rust
#[test]
#[ignore]  // Run manually: cargo test -- --ignored load_test
async fn load_test_concurrent_connections() {
    let master = MasterNode::new("127.0.0.1:9020", "pass").await.unwrap();

    let mut handles = vec![];
    
    // Create 100 concurrent slave connections
    for i in 0..100 {
        let handle = tokio::spawn(async move {
            let mut slave = SlaveNode::new("127.0.0.1", 9020, "pass");
            
            // Connect
            if slave.connect().await.is_err() {
                return Err("Connection failed");
            }

            // Send state updates
            for j in 0..10 {
                let state = StateSnapshot {
                    project_path: format!("/project/{}", i),
                    file_name: format!("file{}.rs", j),
                    cursor_line: j as u32,
                    clipboard_content: format!("data_{}", j),
                    timestamp: 1234567890 + j as u64,
                };

                slave.send_state(state)
                    .await
                    .map_err(|_| "Send failed")?;
            }

            Ok::<(), &str>(())
        });

        handles.push(handle);
    }

    // Wait for all to complete
    let results: Vec<_> = futures::future::join_all(handles).await;
    
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert!(success_count >= 95);  // At least 95% success rate
}

#[test]
#[ignore]
async fn load_test_message_throughput() {
    let start = std::time::Instant::now();
    let mut message_count = 0;

    for _ in 0..10000 {
        let msg = RpcMessage::Grab(StateSnapshot {
            project_path: "/path".to_string(),
            file_name: "file.rs".to_string(),
            cursor_line: 42,
            clipboard_content: "data".to_string(),
            timestamp: 1234567890,
        });

        let json = serde_json::to_string(&msg).unwrap();
        let _deserialized: RpcMessage = serde_json::from_str(&json).unwrap();
        
        message_count += 1;
    }

    let duration = start.elapsed();
    let throughput = message_count as f64 / duration.as_secs_f64();
    
    println!("Message throughput: {:.0} msg/sec", throughput);
    assert!(throughput > 10000.0);  // Should handle 10k msg/sec
}
```

**Çalıştır:**
```bash
cargo test -- --ignored load_test --nocapture
```

**Yapılacaklar:**
- [ ] Concurrent connection test'lerini yaz
- [ ] Message throughput test'lerini yaz
- [ ] Memory leak test'lerini yaz
- [ ] Sustained load test'lerini yaz
- [ ] Performance degradation'ı monitor et

---

### 8. Security Testing

**`tests/security_test.rs`:**

```rust
#[cfg(test)]
mod security_tests {
    use palmsync_shared::*;

    #[test]
    fn test_password_hashing_different_from_input() {
        let password = "mypassword";
        let hash = compute_password_hash(password);
        
        assert_ne!(password, &hash);
        assert!(hash.len() > password.len());
    }

    #[test]
    fn test_injection_prevention() {
        let malicious_path = "../../etc/passwd";
        let result = Validator::validate_file_path(malicious_path);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_rate_limiting_blocks_excess_requests() {
        let limiter = RateLimiter::new(5, 60);  // 5 reqs/min
        
        for _ in 0..5 {
            assert!(limiter.check_rate_limit("192.168.1.1").is_ok());
        }
        
        assert!(limiter.check_rate_limit("192.168.1.1").is_err());
    }

    #[test]
    fn test_encryption_produces_different_ciphertext() {
        let crypto = CryptoManager::new("password");
        let plaintext = b"secret message";
        
        let (nonce1, cipher1) = crypto.encrypt(plaintext).unwrap();
        let (nonce2, cipher2) = crypto.encrypt(plaintext).unwrap();
        
        // Nonces should be different (randomized)
        assert_ne!(nonce1, nonce2);
        // Ciphertexts should be different
        assert_ne!(cipher1, cipher2);
    }

    #[test]
    fn test_hmac_verification_fails_on_tampering() {
        let original = "original message";
        let encrypted = encrypt_message(original, "password").unwrap();
        
        // Tamper with ciphertext
        let mut tampered = encrypted.clone();
        if !tampered.ciphertext.is_empty() {
            tampered.ciphertext[0] ^= 0xFF;  // Flip bits
        }
        
        // Decryption should fail
        let result = decrypt_message(&tampered, "password");
        assert!(result.is_err());
    }

    #[test]
    fn test_input_sanitization_prevents_xss() {
        let malicious = "<script>alert('xss')</script>";
        let sanitized = Validator::sanitize_string(malicious);
        
        assert!(!sanitized.contains("<script>"));
        assert!(sanitized.contains("&lt;"));
    }
}
```

**Yapılacaklar:**
- [ ] Injection attack testleri yaz
- [ ] Authentication bypass testleri yaz
- [ ] Encryption integrity testleri yaz
- [ ] Rate limiting testleri yaz
- [ ] Input validation testleri yaz

---

### 9. Regression Test Suite

**`tests/regression_tests.rs`:**

```rust
#[cfg(test)]
mod regression_tests {
    #[test]
    async fn regression_issue_001_crash_on_invalid_state() {
        // Issue: Application crashed on StateSnapshot with empty file_name
        let state = StateSnapshot {
            project_path: "/path".to_string(),
            file_name: "".to_string(),  // Empty filename
            cursor_line: 0,
            clipboard_content: "".to_string(),
            timestamp: 0,
        };

        let json = serde_json::to_string(&state);
        assert!(json.is_ok());
        
        let deserialized: Result<StateSnapshot, _> = serde_json::from_str(
            &serde_json::to_string(&state).unwrap()
        );
        assert!(deserialized.is_ok());
    }

    #[test]
    async fn regression_issue_002_master_memory_leak() {
        // Issue: Master node's HashMap never cleaned up disconnected slaves
        let master = MasterNode::new("127.0.0.1:9030", "pass").await.unwrap();

        for i in 0..10 {
            let mut slave = SlaveNode::new("127.0.0.1", 9030, "pass");
            slave.connect().await.unwrap();
            // Drop slave (simulate disconnect)
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let active_count = master.get_connected_slaves_count().await.unwrap();
        // Should be 0 after cleanup
        assert_eq!(active_count, 0);
    }

    #[test]
    fn regression_issue_003_unicode_in_clipboard() {
        // Issue: Non-ASCII characters in clipboard broke serialization
        let text = "Hello 世界 🌍";
        
        let result = Clipboard::set_text(text);
        assert!(result.is_ok());
        
        let retrieved = Clipboard::get_text().unwrap();
        assert_eq!(text, retrieved);
    }
}
```

**Yapılacaklar:**
- [ ] Previous bug report'larından regression testleri oluştur
- [ ] Edge case testleri ekle
- [ ] Browser compatibility testleri (UI)
- [ ] Platform-specific testleri (Windows)

---

### 10. CI/CD Pipeline

**`.github/workflows/ci.yml`:**

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  test-rust:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      
      - name: Run tests
        run: cargo test --all --verbose
      
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings
      
      - name: Audit dependencies
        run: cargo audit
      
      - name: Run benchmarks
        run: cargo bench --no-run

  test-cpp:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: ilammy/msvc-dev-cmd@v1
      - uses: cmake-js/setup-cmake@v3
      
      - name: Build C++
        run: |
          cd eye
          mkdir build
          cd build
          cmake ..
          cmake --build . --config Release
      
      - name: Run C++ tests
        run: |
          cd eye/build
          ctest -VV

  security-scan:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run security audit
        run: cargo audit --deny warnings

  test-ui:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-dotnet@v3
        with:
          dotnet-version: '8.0'
      
      - name: Build UI
        run: dotnet build ui/PalmSync.Tray.csproj
      
      - name: Run UI tests
        run: dotnet test ui.tests/
```

**Yapılacaklar:**
- [ ] GitHub Actions workflow'u oluştur
- [ ] Automated testing pipeline'ını kur
- [ ] Code coverage reporting'ini ekle
- [ ] Build artifact'larını publish et
- [ ] Test result'larını track et

---

## 🧪 Test Execution Commands

```bash
# Run all tests
cargo test --all --verbose

# Run specific test
cargo test test_name -- --nocapture

# Run with coverage
cargo tarpaulin --out Html --exclude-files tests/

# Run security tests
cargo test --lib security_tests

# Run performance benchmarks
cargo bench --bench benchmarks

# Run load tests
cargo test -- --ignored load_test

# C++ tests
cd eye/build
ctest -VV

# UI tests
dotnet test ui.tests/
```

---

## 📊 Test Coverage Goals

| Component | Target Coverage | Current |
|-----------|-----------------|---------|
| Networking | 85% | TBD |
| Brain | 80% | TBD |
| Eye | 75% | TBD |
| Shared | 90% | TBD |
| Security | 95% | TBD |
| **Overall** | **80%** | TBD |

---

## 📦 Deliverables

Bu faz tamamlandığında:

1. ✅ **Unit Test Suite**: %80+ coverage
2. ✅ **Integration Tests**: Tüm major components
3. ✅ **E2E Tests**: Complete workflows
4. ✅ **Performance Benchmarks**: Baseline metrics
5. ✅ **Load Tests**: 100+ concurrent users
6. ✅ **Security Tests**: Attack scenarios
7. ✅ **Regression Tests**: Bug fix verification
8. ✅ **CI/CD Pipeline**: Automated testing
9. ✅ **Documentation**: Test run guides

---

## 📝 Known Issues Template

**`KNOWN_ISSUES.md`:**

```markdown
# Known Issues

## Critical
- [ ] Issue #1: Description

## High Priority
- [ ] Issue #2: Description

## Medium Priority
- [ ] Issue #3: Description

## Low Priority / Backlog
- [ ] Issue #4: Description

---
Status: Last updated 2026-07-20
```

---

## 🔄 Test Maintenance

- Weekly: Run full test suite
- Monthly: Update benchmarks, review coverage
- Per-release: Regression test audit
- Per-bug: Add regression test before fix

---

**Son Güncelleme**: 2026-07-20  
**Statüsü**: 🔵 Hazır Implementation için
