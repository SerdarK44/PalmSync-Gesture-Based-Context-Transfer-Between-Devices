# 🌐 Faz 1: Temel Altyapı ve Networking (Foundation)

**Süre**: 2-3 hafta | **Öncelik**: ⭐⭐⭐ (Kritik) | **Bağımlılık**: Yok

---

## 📍 Hedef

Bu faz, PalmSync'in omurgasını oluşturur. Master/Slave topolojisinde çalışan saf P2P ağ altyapısını kuracağız. Cihazlar arasındaki güvenli iletişim, dinamik düğüm yönetimi ve kripto handshake mekanizmasını hayata geçireceğiz.

---

## ✅ Başarı Kriterleri

- [ ] Tailscale mesh VPN entegrasyonu tamamlanmıştır
- [ ] Master node başlatılır ve WebSocket server dinler
- [ ] Slave node'lar Master'a dinamik olarak bağlanırlar
- [ ] SHA-256 handshake protokolü çalışır
- [ ] Düğüm pool'u (Arc<Mutex<HashMap>>) thread-safe şekilde yönetilir
- [ ] Ağ hataları zarif şekilde işlenir (reconnection logic)
- [ ] Temel WebSocket RPC testleri geçer

---

## 🎯 Görevler (Checklist)

### 1. Proje Yapısını Oluştur
```
palmsync/
├── Cargo.toml                 # Workspace konfigurasyonu
├── brain/                     # Rust daemon (Faz 3 için hazırlık)
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── eye/                       # C++ görüntü işleme (Faz 2 için hazırlık)
│   ├── CMakeLists.txt
│   └── src/
├── shared/                    # Ortak veri yapıları ve serileştirme
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── messages.rs
│   │   └── config.rs
├── networking/                # ⭐ FOKÜSTEBİLİR ALAN (Bu faz)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── master.rs
│   │   ├── slave.rs
│   │   ├── handshake.rs
│   │   ├── rpc.rs
│   │   └── utils.rs
│   └── tests/
│       ├── integration_test.rs
│       └── handshake_test.rs
└── documents/
    └── faz-1-networking.md
```

**Yapılacaklar:**
- [ ] Cargo workspace `Cargo.toml` oluştur
- [ ] `networking` crate'i yarat
- [ ] `shared` crate'i veri modelleri için yarat
- [ ] `brain` ve `eye` klasörleri yer tutucu olarak oluştur

---

### 2. Bağımlılıkları Tanımla

**`networking/Cargo.toml` içine ekle:**

```toml
[package]
name = "palmsync-networking"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.35", features = ["full"] }
tokio-tungstenite = "0.21"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"
hex = "0.4"
log = "0.4"
env_logger = "0.11"
anyhow = "1.0"
thiserror = "1.0"
uuid = { version = "1.6", features = ["v4", "serde"] }
parking_lot = "0.12"

[dev-dependencies]
tokio-test = "0.4"
```

**`shared/Cargo.toml`:**

```toml
[package]
name = "palmsync-shared"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.6", features = ["v4", "serde"] }
```

**Yapılacaklar:**
- [ ] Tokio async runtime konfigürasyonunu ayarla
- [ ] Serde JSON serialization'ı kur
- [ ] Logging infrastructure'ı başlat (env_logger)
- [ ] Bağımlılıkları doğrula: `cargo build`

---

### 3. Veri Modelleri Tanımla

**`shared/src/messages.rs`:**

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub node_id: String,
    pub node_type: NodeType,
    pub password_hash: String,  // SHA-256 hash
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub accepted: bool,
    pub message: String,
    pub master_node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Master,
    Slave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub project_path: String,
    pub file_name: String,
    pub cursor_line: u32,
    pub clipboard_content: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcMessage {
    Grab(StateSnapshot),
    Drop(StateSnapshot),
    Ping,
    Pong,
    NodeJoined { node_id: String },
    NodeLeft { node_id: String },
    Error { code: i32, message: String },
}
```

**Yapılacaklar:**
- [ ] Message enum'ları define et
- [ ] StateSnapshot yapısını kur
- [ ] Serialization test et: `cargo test --lib`

---

### 4. Master Node Implementasyonu

**`networking/src/master.rs`:**

Görevler:
- [ ] `MasterNode` struct'ı oluştur (Arc<Mutex<HashMap<String, Slave>>>)
- [ ] WebSocket server dinle (`127.0.0.1:9001` veya Tailscale IP'de)
- [ ] İncoming bağlantıları kabul et
- [ ] Handshake protokolünü implement et
- [ ] State pool'unu yönet (Last-Write-Wins)
- [ ] Slave register/unregister işlerini kur

**Pseudocode:**
```rust
pub struct MasterNode {
    slaves: Arc<Mutex<HashMap<String, SlaveConnection>>>,
    state_storage: Arc<Mutex<StateSnapshot>>,
    password_hash: String,
}

impl MasterNode {
    pub async fn start(&self) -> Result<()> {
        // WebSocket server başlat
        // Gelen bağlantıları dinle
        // Handshake yap
        // Slave registry'e ekle
    }
    
    pub async fn broadcast_state(&self, snapshot: StateSnapshot) -> Result<()> {
        // Tüm bağlı slave'lere durumu gönder
    }
    
    pub async fn remove_slave(&self, node_id: &str) -> Result<()> {
        // Çıkan node'u kaldır
    }
}
```

---

### 5. Slave Node Implementasyonu

**`networking/src/slave.rs`:**

Görevler:
- [ ] `SlaveNode` struct'ı oluştur
- [ ] Master'a WebSocket bağlantısı başlat
- [ ] Handshake request gönder
- [ ] Connection pool'unu yönet
- [ ] Incoming RPC mesajlarını dinle
- [ ] Reconnection logic'i implement et

**Pseudocode:**
```rust
pub struct SlaveNode {
    node_id: String,
    master_addr: String,
    password: String,
    connection: Arc<Mutex<Option<WebSocket>>>,
}

impl SlaveNode {
    pub async fn connect(&mut self) -> Result<()> {
        // Master'a bağlan
        // Handshake yap
        // Event loop başlat
    }
    
    pub async fn send_state(&self, snapshot: StateSnapshot) -> Result<()> {
        // Master'a state snapshot gönder
    }
    
    pub async fn request_state(&self) -> Result<StateSnapshot> {
        // Master'dan en son state'i çek
    }
}
```

---

### 6. Handshake Protokolü

**`networking/src/handshake.rs`:**

Görevler:
- [ ] SHA-256 hash fonksiyonu yaz
- [ ] `HandshakeRequest` oluştur
- [ ] `HandshakeResponse` doğrula
- [ ] Başarısız handshake'i reject et

```rust
use sha2::{Sha256, Digest};

pub fn compute_password_hash(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn verify_handshake(received_hash: &str, expected_hash: &str) -> bool {
    received_hash == expected_hash
}
```

---

### 7. RPC Dispatcher

**`networking/src/rpc.rs`:**

Görevler:
- [ ] Gelen JSON mesajlarını deserialize et
- [ ] RPC handler'ları yönlendir
- [ ] Error response'larını oluştur
- [ ] Async message queue'sü kur

```rust
pub struct RpcDispatcher;

impl RpcDispatcher {
    pub async fn handle_message(msg: RpcMessage) -> Result<RpcMessage> {
        match msg {
            RpcMessage::Grab(snapshot) => { /* state güncelle */ },
            RpcMessage::Drop(_) => { /* state çek */ },
            RpcMessage::Ping => Ok(RpcMessage::Pong),
            _ => Err(anyhow::anyhow!("Unknown message")),
        }
    }
}
```

---

### 8. Test Suite'i Oluştur

**`networking/tests/handshake_test.rs`:**

```rust
#[tokio::test]
async fn test_valid_handshake() {
    // Master start et
    // Slave bağlan
    // Handshake başarılı olması assert et
}

#[tokio::test]
async fn test_invalid_password() {
    // Yanlış şifre ile bağlan
    // Reject edilmesini assert et
}

#[tokio::test]
async fn test_slave_registration() {
    // Multiple slave bağlan
    // Registry'de görünmelerini assert et
}
```

**Yapılacaklar:**
- [ ] WebSocket echo test
- [ ] Handshake success/failure tests
- [ ] Concurrent connection tests
- [ ] State update tests
- [ ] Network failure recovery tests

---

### 9. Logging ve Konfigürasyon

**`networking/src/utils.rs`:**

```rust
pub fn init_logging() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
}

pub struct Config {
    pub master_addr: String,
    pub master_port: u16,
    pub password: String,
    pub node_id: String,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            master_addr: std::env::var("PALMSYNC_MASTER_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string()),
            master_port: std::env::var("PALMSYNC_MASTER_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(9001),
            password: std::env::var("PALMSYNC_PASSWORD").unwrap_or_else(|_| "default".to_string()),
            node_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}
```

**Yapılacaklar:**
- [ ] Environment variable'larını oku
- [ ] Logging output'u format et
- [ ] Debug mode toggle'ını kur

---

## 🧪 Doğrulama ve QA

```bash
# Tüm testleri çalıştır
cargo test --workspace

# Networking crate'ini özellikle test et
cargo test -p palmsync-networking -- --nocapture

# Bellek sızıntılarını kontrol et
cargo build --release
valgrind ./target/release/palmsync-networking-test
```

---

## 📦 Deliverables

Bu faz tamamlandığında şunlar hazır olmalı:

1. ✅ **Çalışan Master Node**: WebSocket server, state pool, slave registry
2. ✅ **Çalışan Slave Node**: Master bağlantısı, event loop, state sync
3. ✅ **Güvenli Handshake**: SHA-256 verificat, password protection
4. ✅ **RPC Messaging**: JSON serialize/deserialize, message dispatcher
5. ✅ **Test Coverage**: %80+ test kapsamı
6. ✅ **Dokumentasyon**: API docs, architecture diagrams

---

## 🔄 Sonraki Faz Bağlantıları

- **Faz 2 (The Eye)**: Slave node'lar UDP IPC'den GRAB/DROP sinyallerini alacak
- **Faz 3 (The Brain)**: RPC dispatcher'a VS Code context'i entegre edilecek

---

## 📝 Notlar ve Uyarılar

⚠️ **Önemli Noktalar:**
- Tailscale authentication'ı ön alıp/setup edin
- WebSocket handshake timeout'u 30 saniyeye set edin
- State pool'una (HashMap) update sırasında deadlock'a dikkat edin
- Ağ errorları için exponential backoff implement edin
- Production'da self-signed cert yerine CA cert kullan

🔐 **Güvenlik Özeti:**
- Şifreler plaintext olarak ağda taşınmaz (sadece SHA-256 hash)
- Tailscale'in TLS/1.3 şifrelenmesi ile double protected
- Her bağlantı unique session ID ile tracked

---

**Son Güncelleme**: 2026-07-20  
**Statüsü**: 🔵 Hazır Implementation için
