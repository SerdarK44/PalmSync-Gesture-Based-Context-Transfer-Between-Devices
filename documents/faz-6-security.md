# 🔐 Faz 6: Güvenlik ve Optimizasyon (Security & Performance)

**Süre**: 2 hafta | **Öncelik**: ⭐⭐⭐ (Kritik) | **Bağımlılık**: Faz 1-5

---

## 📍 Hedef

PalmSync sisteminin güvenliği ve performansını maksimuma çıkaracağız. End-to-end şifrelemesi, secure authentication, saldırı karşı koruma ve sistem kaynaklarının optimize edilmesi üzerine yoğunlaşacağız.

---

## ✅ Başarı Kriterleri

- [ ] TLS/SSL encryption tüm bağlantılarda etkin
- [ ] End-to-end encryption (E2EE) state data için
- [ ] HMAC-SHA256 message authentication
- [ ] Secure password hashing (Argon2)
- [ ] Input validation ve sanitization
- [ ] Rate limiting ve DoS protection
- [ ] Secure clipboard handling
- [ ] CPU usage < 5% idle time'da
- [ ] Memory footprint < 100MB
- [ ] Response latency < 50ms (99th percentile)
- [ ] Audit logging tüm critical operations için
- [ ] Security vulnerability assessment tamamlanmış

---

## 🎯 Görevler (Checklist)

### 1. TLS/SSL Setup

**Certificate Generation (Self-Signed veya CA):**

```bash
# Self-signed certificate oluştur (development)
openssl req -x509 -newkey rsa:4096 -keyout palmsync-key.pem -out palmsync-cert.pem -days 365 -nodes -subj "/CN=localhost"

# Production için CA-signed certificate'ı al
# DigiCert, Let's Encrypt, vb. providers'dan
```

**Rust WebSocket Server (TLS):**

```toml
[dependencies]
tokio = { version = "1.35", features = ["full"] }
tokio-tungstenite = "0.21"
tokio-rustls = "0.24"
rustls = "0.21"
```

```rust
use tokio_rustls::TlsAcceptor;
use rustls::{ServerConfig, NoClientAuth};
use std::sync::Arc;

pub struct SecureWebSocketServer {
    tls_config: Arc<ServerConfig>,
}

impl SecureWebSocketServer {
    pub fn new(cert_path: &str, key_path: &str) -> anyhow::Result<Self> {
        let certs = load_certificates(cert_path)?;
        let key = load_private_key(key_path)?;
        
        let mut config = ServerConfig::new(NoClientAuth::boxed());
        config.set_single_cert(certs, key)?;
        
        Ok(Self {
            tls_config: Arc::new(config),
        })
    }
    
    pub async fn start(&self, addr: &str) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let tls_acceptor = TlsAcceptor::from(self.tls_config.clone());
        
        loop {
            let (stream, _) = listener.accept().await?;
            let tls_acceptor = tls_acceptor.clone();
            
            tokio::spawn(async move {
                if let Ok(tls_stream) = tls_acceptor.accept(stream).await {
                    // Handle WebSocket on TLS stream
                }
            });
        }
    }
}

fn load_certificates(path: &str) -> anyhow::Result<Vec<rustls::Certificate>> {
    let mut file = std::fs::File::open(path)?;
    let certs = rustls_pemfile::certs(&mut file)?
        .into_iter()
        .map(rustls::Certificate)
        .collect();
    Ok(certs)
}

fn load_private_key(path: &str) -> anyhow::Result<rustls::PrivateKey> {
    let mut file = std::fs::File::open(path)?;
    let keys = rustls_pemfile::pkcs8_private_keys(&mut file)?;
    Ok(rustls::PrivateKey(keys.into_iter().next().ok_or(anyhow::anyhow!("No key found"))?))
}
```

**Yapılacaklar:**
- [ ] Self-signed veya CA certificate'ı kur
- [ ] TLS 1.3 konfigürasyonunu ayarla
- [ ] Certificate rotation strategy'sini planla
- [ ] OCSP stapling'i ekle (production)
- [ ] Certificate pinning'i implement et

---

### 2. End-to-End Encryption

**`shared/src/crypto.rs`:**

```rust
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use rand::Rng;
use sha2::{Sha256, Digest};

pub struct CryptoManager {
    cipher: Aes256Gcm,
}

impl CryptoManager {
    pub fn new(password: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let key_bytes = hasher.finalize();
        
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes[..32]);
        let cipher = Aes256Gcm::new(key);
        
        Self { cipher }
    }
    
    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; 12] = rng.gen();
        let nonce = aes_gcm::Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = self.cipher.encrypt(nonce, plaintext)?;
        Ok((nonce_bytes.to_vec(), ciphertext))
    }
    
    pub fn decrypt(&self, nonce: &[u8], ciphertext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let nonce = aes_gcm::Nonce::from_slice(&nonce[..12]);
        let plaintext = self.cipher.decrypt(nonce, ciphertext)?;
        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encrypt_decrypt() {
        let manager = CryptoManager::new("test_password");
        let plaintext = b"Secret message";
        
        let (nonce, ciphertext) = manager.encrypt(plaintext).unwrap();
        let decrypted = manager.decrypt(&nonce, &ciphertext).unwrap();
        
        assert_eq!(plaintext, &decrypted[..]);
    }
}
```

**Message Encryption Wrapper:**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedMessage {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub tag: Vec<u8>,  // HMAC for authenticity
}

pub fn encrypt_message(msg: &str, password: &str) -> anyhow::Result<EncryptedMessage> {
    let crypto = CryptoManager::new(password);
    let (nonce, ciphertext) = crypto.encrypt(msg.as_bytes())?;
    
    // HMAC-SHA256 tag oluştur
    let mut mac = hmac::Hmac::<Sha256>::new_from_slice(password.as_bytes())?;
    mac.update(&ciphertext);
    let tag = mac.finalize().into_bytes().to_vec();
    
    Ok(EncryptedMessage {
        nonce,
        ciphertext,
        tag,
    })
}

pub fn decrypt_message(msg: &EncryptedMessage, password: &str) -> anyhow::Result<String> {
    // HMAC verification
    let mut mac = hmac::Hmac::<Sha256>::new_from_slice(password.as_bytes())?;
    mac.update(&msg.ciphertext);
    mac.verify_slice(&msg.tag)?;
    
    let crypto = CryptoManager::new(password);
    let plaintext = crypto.decrypt(&msg.nonce, &msg.ciphertext)?;
    
    Ok(String::from_utf8(plaintext)?)
}
```

**Bağımlılıklar:**
```toml
[dependencies]
aes-gcm = "0.10"
sha2 = "0.10"
hmac = "0.12"
rand = "0.8"
```

**Yapılacaklar:**
- [ ] AES-256-GCM şifrelemesini implement et
- [ ] Nonce generation'ı randomize et
- [ ] HMAC authenticity tag'ı ekle
- [ ] Decryption'da HMAC doğrula
- [ ] Performance benchmark'ı çalıştır

---

### 3. Secure Authentication

**`shared/src/auth.rs`:**

```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand_core::OsRng;

pub struct AuthManager;

impl AuthManager {
    /// Argon2 ile secure password hash oluştur
    pub fn hash_password(password: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)?
            .to_string();
        
        Ok(password_hash)
    }
    
    /// Password doğrula
    pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
        let parsed_hash = PasswordHash::new(hash)?;
        let argon2 = Argon2::default();
        
        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(_) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    
    /// Secure random token oluştur (session)
    pub fn generate_session_token() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::thread_rng();
        
        (0..64)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_password_hashing() {
        let password = "my_secure_password";
        let hash = AuthManager::hash_password(password).unwrap();
        
        assert!(AuthManager::verify_password(password, &hash).unwrap());
        assert!(!AuthManager::verify_password("wrong_password", &hash).unwrap());
    }
}
```

**Bağımlılıklar:**
```toml
[dependencies]
argon2 = "0.5"
rand = "0.8"
```

**Yapılacaklar:**
- [ ] Argon2 password hashing'i implement et
- [ ] Password verification logic'ini kur
- [ ] Session token generation'ı ekle
- [ ] Token expiration'ı implement et
- [ ] Rate limiting on auth attempts

---

### 4. Input Validation ve Sanitization

**`shared/src/validation.rs`:**

```rust
pub struct Validator;

impl Validator {
    /// File path'ı validate et (traversal attacks'ı prevent et)
    pub fn validate_file_path(path: &str) -> anyhow::Result<()> {
        if path.contains("..") || path.contains("~") {
            return Err(anyhow::anyhow!("Invalid file path: contains dangerous characters"));
        }
        
        if path.starts_with("/") || path[1..].starts_with(":") {
            return Err(anyhow::anyhow!("Absolute paths not allowed"));
        }
        
        Ok(())
    }
    
    /// IP address'ı validate et
    pub fn validate_ip_address(ip: &str) -> anyhow::Result<()> {
        match ip.parse::<std::net::IpAddr>() {
            Ok(_) => Ok(()),
            Err(_) => Err(anyhow::anyhow!("Invalid IP address")),
        }
    }
    
    /// Port number'ını validate et
    pub fn validate_port(port: u16) -> anyhow::Result<()> {
        if port < 1024 {
            Err(anyhow::anyhow!("Port < 1024 requires admin privileges"))
        } else if port > 65535 {
            Err(anyhow::anyhow!("Port > 65535 invalid"))
        } else {
            Ok(())
        }
    }
    
    /// String'i sanitize et (XSS prevention)
    pub fn sanitize_string(input: &str) -> String {
        input
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
            .replace("'", "&#39;")
            .replace("&", "&amp;")
    }
    
    /// JSON malformed check
    pub fn validate_json(json_str: &str) -> anyhow::Result<()> {
        serde_json::from_str::<serde_json::Value>(json_str)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_path_traversal_prevention() {
        assert!(Validator::validate_file_path("../../etc/passwd").is_err());
        assert!(Validator::validate_file_path("myfile.txt").is_ok());
    }
    
    #[test]
    fn test_port_validation() {
        assert!(Validator::validate_port(80).is_ok());
        assert!(Validator::validate_port(22).is_err());  // Admin required
        assert!(Validator::validate_port(65536).is_err());
    }
}
```

**Yapılacaklar:**
- [ ] Input length limit'lerini kur
- [ ] Path traversal attacks'ı prevent et
- [ ] Injection attacks'ı sanitize et
- [ ] JSON validation'ı implement et
- [ ] Whitelist approach kullan (blacklist değil)

---

### 5. Rate Limiting ve DoS Protection

**`networking/src/rate_limiter.rs`:**

```rust
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use std::sync::Arc;

pub struct RateLimiter {
    /// (IP Address) -> (request count, window start time)
    windows: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    max_requests_per_window: u32,
    window_duration: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            windows: Arc::new(Mutex::new(HashMap::new())),
            max_requests_per_window: max_requests,
            window_duration: Duration::from_secs(window_secs),
        }
    }
    
    pub fn check_rate_limit(&self, client_ip: &str) -> anyhow::Result<()> {
        let mut windows = self.windows.lock();
        let now = Instant::now();
        
        match windows.get_mut(client_ip) {
            Some((count, window_start)) => {
                if now.duration_since(*window_start) > self.window_duration {
                    // Window reset
                    windows.insert(client_ip.to_string(), (1, now));
                } else if *count >= self.max_requests_per_window {
                    return Err(anyhow::anyhow!("Rate limit exceeded"));
                } else {
                    *count += 1;
                }
            }
            None => {
                windows.insert(client_ip.to_string(), (1, now));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rate_limiting() {
        let limiter = RateLimiter::new(5, 60);  // 5 reqs per 60 secs
        
        for _ in 0..5 {
            assert!(limiter.check_rate_limit("192.168.1.1").is_ok());
        }
        
        assert!(limiter.check_rate_limit("192.168.1.1").is_err());
    }
}
```

**WebSocket handler'ında kullan:**

```rust
pub async fn handle_websocket_connection(
    stream: TcpStream,
    rate_limiter: Arc<RateLimiter>,
    peer_addr: SocketAddr,
) -> anyhow::Result<()> {
    rate_limiter.check_rate_limit(&peer_addr.ip().to_string())?;
    
    // ... rest of handler
    Ok(())
}
```

**Yapılacaklar:**
- [ ] Per-client rate limiting'i implement et
- [ ] Token bucket algorithm'ı ekle
- [ ] Connection timeout'ları set et
- [ ] Max payload size'ı limit et
- [ ] Backoff strategy'i implement et

---

### 6. Secure Clipboard Handling

**`brain/src/clipboard.rs` (Enhanced):**

```rust
#[cfg(target_os = "windows")]
pub struct SecureClipboard;

#[cfg(target_os = "windows")]
impl SecureClipboard {
    const MAX_CLIPBOARD_SIZE: usize = 10 * 1024 * 1024;  // 10MB max
    
    /// Clipboard'ı güvenli şekilde oku ve şifrele
    pub fn get_encrypted_text(password: &str) -> anyhow::Result<Vec<u8>> {
        use clipboard_win::{formats, get_clipboard};
        
        let data = get_clipboard::<formats::Unicode>()?;
        
        // Size check
        if data.len() > Self::MAX_CLIPBOARD_SIZE {
            return Err(anyhow::anyhow!("Clipboard content too large"));
        }
        
        // Sanitize (sensitive data için optional mask)
        let sanitized = Self::sanitize_clipboard_content(&data)?;
        
        // Encrypt
        let crypto = crate::crypto::CryptoManager::new(password);
        let (nonce, ciphertext) = crypto.encrypt(sanitized.as_bytes())?;
        
        let mut result = Vec::new();
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
    
    /// Şifreli clipboard'ı decrypt et ve yaz
    pub fn set_encrypted_text(encrypted: &[u8], password: &str) -> anyhow::Result<()> {
        use clipboard_win::{formats, set_clipboard};
        
        if encrypted.len() < 12 {
            return Err(anyhow::anyhow!("Invalid encrypted data"));
        }
        
        let (nonce, ciphertext) = encrypted.split_at(12);
        
        let crypto = crate::crypto::CryptoManager::new(password);
        let plaintext = crypto.decrypt(nonce, ciphertext)?;
        let text = String::from_utf8(plaintext)?;
        
        set_clipboard(formats::Unicode, text)?;
        Ok(())
    }
    
    fn sanitize_clipboard_content(content: &str) -> anyhow::Result<String> {
        // Potentially remove sensitive patterns
        // - Credit card numbers
        // - API keys
        // - Passwords
        
        if content.contains("BEGIN PRIVATE KEY") {
            log::warn!("Sensitive content detected in clipboard");
            // Consider masking or requiring confirmation
        }
        
        Ok(content.to_string())
    }
    
    /// Clipboard'ı güvenli şekilde temizle
    pub fn clear_secure() -> anyhow::Result<()> {
        use clipboard_win::{formats, set_clipboard};
        
        // Overwrite with random data multiple times (Gutmann)
        for _ in 0..3 {
            let random: String = (0..1000)
                .map(|_| "X")
                .collect();
            set_clipboard(formats::Unicode, random)?;
        }
        
        // Finally set to empty
        set_clipboard(formats::Unicode, "")?;
        Ok(())
    }
}
```

**Yapılacaklar:**
- [ ] Max clipboard size'ı limit et
- [ ] Sensitive data detection'ı implement et
- [ ] Encryption/decryption'ı add et
- [ ] Secure wipe'ı implement et
- [ ] Clipboard access logging'i kur

---

### 7. Audit Logging

**`shared/src/audit.rs`:**

```rust
use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;

pub struct AuditLogger {
    log_path: String,
}

impl AuditLogger {
    pub fn new(log_path: &str) -> Self {
        Self {
            log_path: log_path.to_string(),
        }
    }
    
    pub fn log_authentication(user_id: &str, success: bool) {
        let entry = format!(
            "{} [AUTH] user_id={} success={}",
            Local::now().to_rfc3339(),
            user_id,
            success
        );
        Self::write_log_entry(&entry);
    }
    
    pub fn log_state_sync(node_id: &str, operation: &str, status: &str) {
        let entry = format!(
            "{} [SYNC] node_id={} operation={} status={}",
            Local::now().to_rfc3339(),
            node_id,
            operation,
            status
        );
        Self::write_log_entry(&entry);
    }
    
    pub fn log_security_event(event: &str, details: &str) {
        let entry = format!(
            "{} [SECURITY] event={} details={}",
            Local::now().to_rfc3339(),
            event,
            details
        );
        Self::write_log_entry(&entry);
    }
    
    fn write_log_entry(entry: &str) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("audit.log")
        {
            let _ = writeln!(file, "{}", entry);
        }
    }
}
```

**Yapılacaklar:**
- [ ] Audit log file'ı kur
- [ ] Authentication event'lerini log et
- [ ] Data sync event'lerini log et
- [ ] Security event'lerini log et
- [ ] Log rotation'ı implement et
- [ ] Log integrity'sini protect et (optional: HMAC)

---

### 8. Performance Optimization

**CPU Optimization:**

```rust
// Lazy static'leri kullan
use once_cell::sync::Lazy;

static CRYPTO_MANAGER: Lazy<CryptoManager> = Lazy::new(|| {
    CryptoManager::new("default_password")
});

// Async I/O kullan (blocking call'ları spawn_blocking'de yap)
pub async fn read_large_file(path: &str) -> anyhow::Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        std::fs::read(path)
    })
    .await?
}

// Object pooling (connection reuse)
pub struct ConnectionPool {
    pool: deadpool::managed::Pool<PooledConnection>,
}

impl ConnectionPool {
    pub async fn get_connection(&self) -> anyhow::Result<PooledConnection> {
        Ok(self.pool.get().await?)
    }
}
```

**Memory Optimization:**

```rust
// String allocation'ı minimize et
let mut buffer = String::with_capacity(1024);  // Pre-allocate

// Large object'leri reference olarak pass et
fn process_data(data: &[u8]) -> anyhow::Result<()> {
    // ...
    Ok(())
}

// Memory profiling
use pprof::ProfilerGuard;

pub fn profile_memory() {
    let guard = ProfilerGuard::new(100);
    // ... code to profile
    if let Ok(report) = guard.report().build() {
        println!("{}", report);
    }
}
```

**Latency Optimization:**

```rust
// Batch operations
async fn batch_process_events(events: Vec<Event>) -> anyhow::Result<()> {
    // Process multiple events in parallel
    let futures: Vec<_> = events
        .into_iter()
        .map(|event| process_event(event))
        .collect();
    
    futures::future::try_join_all(futures).await?;
    Ok(())
}

// Caching strategy
use lru::LruCache;

pub struct StateCache {
    cache: Mutex<LruCache<String, StateSnapshot>>,
}

impl StateCache {
    pub fn get(&self, key: &str) -> Option<StateSnapshot> {
        self.cache.lock().get(key).cloned()
    }
    
    pub fn put(&self, key: String, value: StateSnapshot) {
        self.cache.lock().put(key, value);
    }
}
```

**Bağımlılıklar:**
```toml
[dependencies]
once_cell = "1.19"
deadpool = "0.9"
pprof = "0.13"
lru = "0.12"
```

**Yapılacaklar:**
- [ ] CPU profiling'i çalıştır
- [ ] Hot path'leri identify et
- [ ] Object allocation'ı minimize et
- [ ] Connection pooling'i implement et
- [ ] Caching strategy'sini kur
- [ ] Benchmark'larını yazı

---

### 9. Security Audit Checklist

**Yapılacaklar (Security Review):**

- [ ] Code static analysis (clippy, cargo-audit)
- [ ] Dependency vulnerability scan (cargo-audit)
- [ ] Penetration testing (manual + automated)
- [ ] Memory safety check (MIRI, valgrind)
- [ ] Cryptographic review (crypto expert)
- [ ] Threat modeling completion
- [ ] Secure coding guidelines documentation

```bash
# Rust security tools
cargo clippy -- -D warnings
cargo audit
cargo outdated

# Memory safety check
cargo +nightly miri test
```

---

### 10. Security Documentation

**`SECURITY.md`:**

```markdown
# Security Policy

## Reporting Vulnerabilities

Please report security vulnerabilities to: security@palmsync.local

## Known Security Measures

- TLS 1.3 encryption for all network communications
- End-to-end encryption for state data (AES-256-GCM)
- Argon2 password hashing
- HMAC-SHA256 message authentication
- Input validation and sanitization
- Rate limiting and DoS protection
- Audit logging of critical operations

## Security Roadmap

- [ ] Certificate pinning
- [ ] Hardware security module support
- [ ] Multi-factor authentication
- [ ] Fine-grained access controls

## Testing Recommendations

- Regular penetration testing
- Dependency vulnerability scanning
- Cryptographic protocol review
- Performance under attack conditions
```

**Yapılacaklar:**
- [ ] Security policy document'ını oluştur
- [ ] Vulnerability disclosure process'ini tanımla
- [ ] Security update schedule'ı belirle
- [ ] Incident response plan'ını yaz

---

## 🧪 Security Testing

```bash
# Fuzzing
cargo-fuzz setup
cargo +nightly fuzz run fuzz_crypto

# Dependency audit
cargo audit --deny warnings

# Code review
cargo clippy --all-targets --all-features -- -D warnings

# Performance benchmarks
cargo bench --bench security_benchmarks
```

---

## 📦 Deliverables

Bu faz tamamlandığında:

1. ✅ **TLS/SSL Encryption**: Tüm ağ bağlantılarında
2. ✅ **End-to-End Encryption**: State data için AES-256-GCM
3. ✅ **Secure Authentication**: Argon2 password hashing
4. ✅ **Input Validation**: Injection attack protection
5. ✅ **Rate Limiting**: DoS protection
6. ✅ **Audit Logging**: Critical operations logging
7. ✅ **Performance Optimized**: < 50ms latency, < 5% CPU
8. ✅ **Security Documentation**: Policy ve guidelines

---

## 🔄 Sonraki Faz Bağlantıları

- **Faz 7 (Testing)**: Security testing ve penetration testing

---

## 📝 Uyarılar

⚠️ **Güvenlik Notları:**
- Cryptographic keys'i securely store et (HSM ideal)
- Regular security audit'ları schedule et
- Dependency update'lerini monitör et
- Production certificate'ları self-signed yerine CA-signed kullan
- Log retention policy'ni GDPR ile uyumlu tut

🔐 **Best Practices:**
- Principle of least privilege
- Defense in depth
- Assume compromise
- Continuous monitoring
- Incident response planning

---

**Son Güncelleme**: 2026-07-20  
**Statüsü**: 🔵 Hazır Implementation için
