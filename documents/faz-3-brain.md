# 🧠 Faz 3: Çekirdek Sistem ve Bağlam Yönetimi (The Brain)

**Süre**: 2-3 hafta | **Öncelik**: ⭐⭐⭐ (Kritik) | **Bağımlılık**: Faz 1 + Faz 2

---

## 📍 Hedef

Rust'ta yazılı olan sistem özü (daemon) geliştireceğiz. Bu servis Faz 2'nin gözünden UDP IPC sinyalleri alacak, Faz 1'in ağ altyapısından state'i yönetecek ve VS Code'u programlı olarak kontrol ederek çalışma alanını canlandıracaktır.

---

## ✅ Başarı Kriterleri

- [ ] Asenkron Tokio event loop çalışır
- [ ] UDP listener'ı `localhost:12345`'de dinler
- [ ] Jest event'lerini (GRAB/DROP) deserialize eder
- [ ] GRAB sırasında aktif pencere, file path ve cursor line'ı yakalar
- [ ] State'i Master düğüme WebSocket üzerinden gönderir
- [ ] DROP sırasında Master'dan en güncel state'i çeker
- [ ] `code --goto` komutu ile VS Code'u açar
- [ ] Clipboard içeriğini güncellemeleri yönetir
- [ ] Error recovery ve reconnection logic'i çalışır
- [ ] Logging ve monitoring infrastructure kurulur

---

## 🎯 Görevler (Checklist)

### 1. Proje Yapısını Tamamla

**`brain/src/` yapısı:**

```
brain/src/
├── main.rs              # Entry point + Windows subsystem
├── lib.rs               # Public API
├── daemon.rs            # Daemon struct ve lifecycle
├── ipc_listener.rs      # UDP listener (localhost:12345)
├── context_capture.rs   # VS Code context extraction
├── state_manager.rs     # State pull/push logic
├── executor.rs          # OS command executor
├── clipboard.rs         # Clipboard operations
├── rpc_client.rs        # WebSocket client (networking crate ile)
└── error.rs             # Error types
```

**Yapılacaklar:**
- [ ] Module structure'ı organize et
- [ ] Workspace'te shared types'ları import et
- [ ] Windows subsystem flag'ını ekle

---

### 2. Windows Subsystem Setup

**`brain/src/main.rs`:**

```rust
#![windows_subsystem = "windows"]

mod daemon;
mod ipc_listener;
mod context_capture;
mod state_manager;
mod executor;
mod clipboard;
mod rpc_client;
mod error;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let daemon = daemon::Daemon::new().await?;
    daemon.run().await?;

    Ok(())
}
```

**Yapılacaklar:**
- [ ] `#![windows_subsystem = "windows"]` ekle
- [ ] Tokio runtime'ı configure et
- [ ] Logging'i initialize et
- [ ] Graceful shutdown signal'larını handle et

---

### 3. UDP IPC Listener

**`brain/src/ipc_listener.rs`:**

```rust
use tokio::net::UdpSocket;
use serde_json::json;
use palmsync_shared::RpcMessage;

pub struct IPCListener {
    socket: UdpSocket,
}

impl IPCListener {
    pub async fn new() -> anyhow::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:12345").await?;
        log::info!("IPC listener started on 127.0.0.1:12345");
        Ok(Self { socket })
    }

    pub async fn listen<F>(&self, mut handler: F) -> anyhow::Result<()>
    where
        F: FnMut(palmsync_shared::RpcMessage) -> futures::future::BoxFuture<'static, ()> + Send,
    {
        let mut buf = vec![0u8; 4096];

        loop {
            match self.socket.recv(&mut buf).await {
                Ok(n) => {
                    if let Ok(json_str) = std::str::from_utf8(&buf[..n]) {
                        if let Ok(msg) = serde_json::from_str::<palmsync_shared::RpcMessage>(json_str) {
                            handler(msg).await;
                        }
                    }
                }
                Err(e) => {
                    log::error!("UDP receive error: {}", e);
                }
            }
        }
    }
}
```

**Yapılacaklar:**
- [ ] UDP socket'i bind et (127.0.0.1:12345)
- [ ] Async buf allocate et
- [ ] JSON deserialize et
- [ ] Handler callback'ini call et
- [ ] Error logging'i implement et

---

### 4. Bağlam Yakalama (Context Capture)

**`brain/src/context_capture.rs`:**

```rust
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VSCodeContext {
    pub project_path: String,
    pub file_name: String,
    pub cursor_line: u32,
    pub cursor_column: u32,
}

pub struct ContextCapture;

impl ContextCapture {
    /// VS Code'dan aktif dosya bilgisini çek
    pub fn capture_vscode_context() -> anyhow::Result<VSCodeContext> {
        // PowerShell komutundan aktif window'u al
        let output = Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                r#"
                [System.Diagnostics.Process]::GetProcessesByName('Code') | 
                Select-Object -First 1 | 
                ForEach-Object {
                    Get-Process -Id $_.Id | 
                    Select-Object @{Name='MainWindowTitle'; Expression={$_.MainWindowTitle}}
                }
                "#,
            ])
            .output()?;

        let window_title = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // VS Code CLI aracını kullanarak açık dosya bilgisini al
        let vscode_output = Command::new("code")
            .args(&["--list-extensions"])
            .output()?;

        // Alternatively: Use Windows API to get active window details
        // This is a simplified approach - in production, use win32-rs or similar

        Ok(VSCodeContext {
            project_path: Self::extract_project_path(&window_title)?,
            file_name: Self::extract_filename(&window_title)?,
            cursor_line: Self::extract_line_number(&window_title).unwrap_or(1),
            cursor_column: 0,
        })
    }

    fn extract_project_path(title: &str) -> anyhow::Result<String> {
        // Parse VS Code window title to extract project path
        // Format: "filename - /path/to/project - Visual Studio Code"
        Ok(title.split(" - ").nth(1).unwrap_or("").to_string())
    }

    fn extract_filename(title: &str) -> anyhow::Result<String> {
        Ok(title.split(" - ").next().unwrap_or("").to_string())
    }

    fn extract_line_number(title: &str) -> Option<u32> {
        // Try to extract line number from title if present
        None
    }

    /// Active foreground window'u kontrol et
    pub fn get_active_window_title() -> anyhow::Result<String> {
        let output = Command::new("powershell")
            .args(&["-NoProfile", "-Command", "Get-Process | Where-Object {$_.MainWindowHandle -ne 0} | Sort-Object -Property MainWindowHandle -Descending | Select-Object -First 1 | Select-Object -ExpandProperty Name"])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
```

**Daha Robust Versyon (Windows API kullanarak):**

Bağımlılıkları Cargo.toml'ye ekle:
```toml
[dependencies]
winapi = { version = "0.3", features = ["winuser"] }
```

```rust
use winapi::um::winuser::{GetForegroundWindow, GetWindowTextW};
use std::os::windows::ffi::OsStrExt;
use std::ffi::OsStr;

pub fn get_active_window_process() -> anyhow::Result<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut title = vec![0u16; 512];
        GetWindowTextW(hwnd, title.as_mut_ptr(), 512);
        
        let len = title.iter().position(|&c| c == 0).unwrap_or(512);
        let title_str = String::from_utf16(&title[..len])?;
        
        Ok(title_str)
    }
}
```

**Yapılacaklar:**
- [ ] Active window'u al (Windows API)
- [ ] VS Code window title'ından data extract et
- [ ] Project path'i parse et
- [ ] File path'i al
- [ ] Cursor position'ı al (VS Code extension aracılığıyla opsiyonel)
- [ ] Error handling ekle

---

### 5. State Manager

**`brain/src/state_manager.rs`:**

```rust
use palmsync_shared::StateSnapshot;
use std::sync::Arc;
use parking_lot::Mutex;
use chrono::Utc;

pub struct StateManager {
    current_state: Arc<Mutex<Option<StateSnapshot>>>,
    rpc_client: crate::rpc_client::RpcClient,
}

impl StateManager {
    pub fn new(rpc_client: crate::rpc_client::RpcClient) -> Self {
        Self {
            current_state: Arc::new(Mutex::new(None)),
            rpc_client,
        }
    }

    /// GRAB: Yerel durumu yakala ve Master'a gönder
    pub async fn grab_state(&self) -> anyhow::Result<()> {
        log::info!("GRAB event triggered");

        // Bağlam yakala
        let context = crate::context_capture::ContextCapture::capture_vscode_context()?;
        let clipboard_content = crate::clipboard::Clipboard::get_text()?;

        let snapshot = StateSnapshot {
            project_path: context.project_path,
            file_name: context.file_name,
            cursor_line: context.cursor_line,
            clipboard_content,
            timestamp: Utc::now().timestamp_millis() as u64,
        };

        // State'i sakla
        *self.current_state.lock() = Some(snapshot.clone());

        // Master'a gönder
        self.rpc_client.send_grab(snapshot).await?;

        Ok(())
    }

    /// DROP: Master'dan durumu çek ve uygula
    pub async fn drop_state(&self) -> anyhow::Result<()> {
        log::info!("DROP event triggered");

        // Master'dan en güncel state'i al
        let snapshot = self.rpc_client.request_state().await?;

        // Clipboard'ı güncelle
        crate::clipboard::Clipboard::set_text(&snapshot.clipboard_content)?;

        // VS Code'u aç/switch et
        crate::executor::Executor::open_vscode(
            &snapshot.project_path,
            &snapshot.file_name,
            snapshot.cursor_line,
        )
        .await?;

        // State'i sakla
        *self.current_state.lock() = Some(snapshot);

        Ok(())
    }

    pub fn get_current_state(&self) -> Option<StateSnapshot> {
        self.current_state.lock().clone()
    }
}
```

**Yapılacaklar:**
- [ ] Current state'i thread-safe sakla (Arc<Mutex>)
- [ ] GRAB logic'ini implement et
- [ ] DROP logic'ini implement et
- [ ] State timestamping'ini ekle
- [ ] RPC client'i integrate et

---

### 6. Command Executor

**`brain/src/executor.rs`:**

```rust
use tokio::process::Command;
use std::time::Duration;

pub struct Executor;

impl Executor {
    /// VS Code'u belirli dosya ve satırda aç
    pub async fn open_vscode(
        project_path: &str,
        file_name: &str,
        line_number: u32,
    ) -> anyhow::Result<()> {
        let file_path = format!("{}\\{}", project_path, file_name);

        let cmd = format!("code --goto \"{}:{}\"", file_path, line_number);

        log::info!("Executing: {}", cmd);

        let output = Command::new("cmd")
            .args(&["/C", &cmd])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::error!("VS Code command failed: {}", stderr);
            return Err(anyhow::anyhow!("Failed to open VS Code: {}", stderr));
        }

        // Wait for VS Code to open/switch
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(())
    }

    /// Komut çalıştır ve output'u döndür
    pub async fn run_command(cmd: &str) -> anyhow::Result<String> {
        let output = Command::new("cmd")
            .args(&["/C", cmd])
            .output()
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
```

**Yapılacaklar:**
- [ ] `code --goto` command'ini çalıştır
- [ ] Process output'unu handle et
- [ ] Error'ları log et
- [ ] Timeout'u set et (30 saniye)
- [ ] Success/failure return'ü kontrol et

---

### 7. Clipboard Manager

**`brain/src/clipboard.rs`:**

```rust
#[cfg(target_os = "windows")]
pub struct Clipboard;

#[cfg(target_os = "windows")]
impl Clipboard {
    pub fn get_text() -> anyhow::Result<String> {
        use clipboard_win::{formats, get_clipboard};

        let data = get_clipboard::<formats::Unicode>()?;
        Ok(data)
    }

    pub fn set_text(content: &str) -> anyhow::Result<()> {
        use clipboard_win::{formats, set_clipboard};

        set_clipboard(formats::Unicode, content)?;
        Ok(())
    }
}
```

**Bağımlılık:**
```toml
[dependencies]
clipboard-win = "4.5"
```

**Yapılacaklar:**
- [ ] Windows Clipboard API'yi kullan
- [ ] Get operation'ı implement et
- [ ] Set operation'ı implement et
- [ ] Error handling ekle
- [ ] UTF-8 encoding'i handle et

---

### 8. WebSocket RPC Client

**`brain/src/rpc_client.rs`:**

```rust
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, WebSocketStream};
use palmsync_shared::{RpcMessage, StateSnapshot};

pub struct RpcClient {
    master_addr: String,
    ws: Option<WebSocketStream<TcpStream>>,
}

impl RpcClient {
    pub fn new(master_addr: &str) -> Self {
        Self {
            master_addr: master_addr.to_string(),
            ws: None,
        }
    }

    pub async fn connect(&mut self) -> anyhow::Result<()> {
        let url = format!("ws://{}", self.master_addr);
        log::info!("Connecting to Master at {}", url);

        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                log::info!("Connected to Master");
                self.ws = Some(ws_stream);
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to connect to Master: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn send_grab(&mut self, snapshot: StateSnapshot) -> anyhow::Result<()> {
        if self.ws.is_none() {
            self.connect().await?;
        }

        let msg = RpcMessage::Grab(snapshot);
        let json = serde_json::to_string(&msg)?;

        if let Some(ws) = &mut self.ws {
            ws.send(tokio_tungstenite::tungstenite::Message::Text(json))
                .await?;
        }

        Ok(())
    }

    pub async fn request_state(&mut self) -> anyhow::Result<StateSnapshot> {
        if self.ws.is_none() {
            self.connect().await?;
        }

        // Ping ata ve yanıt bekle
        if let Some(ws) = &mut self.ws {
            ws.send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::to_string(&RpcMessage::Ping)?,
            ))
            .await?;

            // TODO: State'i al ve deserialize et
            // Bu proseste Master'dan en son snapshot'u çekmeli
        }

        Err(anyhow::anyhow!("Not implemented yet"))
    }
}
```

**Yapılacaklar:**
- [ ] WebSocket connection'ı kur
- [ ] Async message send/receive implement et
- [ ] Auto-reconnect logic'i ekle
- [ ] Heartbeat/ping-pong implement et
- [ ] Message queue'sini yönet

---

### 9. Daemon Koordinatörü

**`brain/src/daemon.rs`:**

```rust
use crate::ipc_listener::IPCListener;
use crate::state_manager::StateManager;
use crate::rpc_client::RpcClient;
use palmsync_shared::RpcMessage;

pub struct Daemon {
    ipc_listener: IPCListener,
    state_manager: StateManager,
}

impl Daemon {
    pub async fn new() -> anyhow::Result<Self> {
        let ipc_listener = IPCListener::new().await?;
        let rpc_client = RpcClient::new("127.0.0.1:9001");
        let state_manager = StateManager::new(rpc_client);

        Ok(Self {
            ipc_listener,
            state_manager,
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        log::info!("PalmSync Brain daemon started");

        let state_mgr = self.state_manager.clone_arc();

        self.ipc_listener
            .listen(move |msg: RpcMessage| {
                let state_mgr_clone = state_mgr.clone();
                Box::pin(async move {
                    match msg {
                        RpcMessage::Grab(_) => {
                            if let Err(e) = state_mgr_clone.grab_state().await {
                                log::error!("GRAB failed: {}", e);
                            }
                        }
                        RpcMessage::Drop(_) => {
                            if let Err(e) = state_mgr_clone.drop_state().await {
                                log::error!("DROP failed: {}", e);
                            }
                        }
                        _ => {}
                    }
                })
            })
            .await
    }
}
```

**Yapılacaklar:**
- [ ] Daemon lifecycle'ı manage et
- [ ] IPC listener'ı başlat
- [ ] Event handler'ları register et
- [ ] Graceful shutdown implement et
- [ ] Signal handling (SIGTERM, SIGINT)

---

### 10. Error Handling

**`brain/src/error.rs`:**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PalmSyncError {
    #[error("IPC Error: {0}")]
    IpcError(String),

    #[error("Context Capture Error: {0}")]
    ContextError(String),

    #[error("Command Execution Error: {0}")]
    ExecutionError(String),

    #[error("Clipboard Error: {0}")]
    ClipboardError(String),

    #[error("Network Error: {0}")]
    NetworkError(String),

    #[error("Serialization Error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
}
```

**Yapılacaklar:**
- [ ] Custom error types'ı define et
- [ ] thiserror crate'ini use et
- [ ] Error propagation'ı handle et
- [ ] Logging'i error'larla integrate et

---

## 🧪 Test Suite

**`brain/tests/integration_test.rs`:**

```rust
#[tokio::test]
async fn test_grab_state_capture() {
    // Mock VS Code context
    // Grab event tetikle
    // State capture'lanmasını verify et
}

#[tokio::test]
async fn test_drop_state_restore() {
    // Master'dan state al
    // VS Code command'ini çalıştır
    // Success'i verify et
}

#[tokio::test]
async fn test_udp_message_parsing() {
    // UDP socket'e JSON message gönder
    // Handler'ın call edilmesini verify et
}
```

**Yapılacaklar:**
- [ ] Async test framework'ü kur
- [ ] Mock objects oluştur
- [ ] Integration testler yaz
- [ ] Error case testleri yaz

---

## 📦 Deliverables

Bu faz tamamlandığında:

1. ✅ **`palmsync-daemon.exe`**: Çalışan Rust daemon
2. ✅ **UDP Listener**: localhost:12345'de çalışan IPC
3. ✅ **Context Capture**: VS Code bağlam yakalaması
4. ✅ **State Management**: GRAB/DROP logic
5. ✅ **Command Execution**: VS Code integration
6. ✅ **Clipboard Management**: Content sync
7. ✅ **WebSocket Client**: Master connection
8. ✅ **Error Handling**: Robust error recovery

---

## 🔄 Sonraki Faz Bağlantıları

- **Faz 4 (Deployment)**: Daemon'ı Windows Service olarak register et
- **Faz 5 (UI)**: System tray application'ı kur

---

## 📝 Uyarılar

⚠️ **Dikkat Edilmesi Gerekenler:**
- Tokio runtime'ında blocking I/O yapma (clipbooard, komutlar `spawn_blocking` kullan)
- WebSocket connection'ı uzun süre kapalı kalırsa reconnect logic'i çalıştır
- VS Code açık değilse `code` command'i yüklü olmalı
- Cursor position bilgisi VS Code extension aracılığıyla daha doğru alınabilir (opsiyonel)
- Windows Clipboard çoğu yazılımla uyumlu ama bazı binary formatlar sorun çıkartabilir

🔐 **Güvenlik:**
- Clipboard'da hassas veriye dikkat et
- RPC message'larını encrypt et (Faz 6)
- Process execution'ı restrict et

---

**Son Güncelleme**: 2026-07-20  
**Statüsü**: 🔵 Hazır Implementation için
