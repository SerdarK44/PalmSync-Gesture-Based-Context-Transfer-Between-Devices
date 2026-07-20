# 🚀 Faz 4: Platform Entegrasyonu ve Dağıtım (Deployment)

**Süre**: 2 hafta | **Öncelik**: ⭐⭐⭐ (Kritik) | **Bağımlılık**: Faz 1 + Faz 2 + Faz 3

---

## 📍 Hedef

PalmSync'i Windows cihazlarında otomatik olarak başlayan bir sistem servisi olarak dağıtacağız. Tek tıkla yükleyici (installer) oluşturup, statik binary'leri hazırlayacak ve startup sırasında otomatik başlatılmasını sağlayacağız.

---

## ✅ Başarı Kriterleri

- [ ] Statik bağlantılı (statically linked) `palmsync-eye.exe` derlenmiş
- [ ] `palmsync-brain.exe` Windows subsystem ile compile edilmiş
- [ ] `palmsync-daemon.exe` servisi kurulabilir ve çalışabilir
- [ ] Inno Setup installer oluşturulmuş
- [ ] Autorun registry entry'leri yazılmış
- [ ] Windows Service installation/uninstallation logic'i çalışır
- [ ] Kurulum sırasında gerekli permissions istenir
- [ ] Uninstall temiz şekilde kaldırır (cleanup)
- [ ] Sistem başlangıcında servis otomatik başlar
- [ ] Toast notifications çalışır

---

## 🎯 Görevler (Checklist)

### 1. Statik Build Setup

**OpenCV Static Linking Konfigürasyonu:**

```cmake
# eye/CMakeLists.txt
set(OpenCV_STATIC ON)
set(BUILD_SHARED_LIBS OFF)

# vcpkg integration
set(VCPKG_TARGET_TRIPLET x64-windows-static)
```

**Rust Release Build:**

```toml
# Cargo.toml [profile.release]
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

**Build Command:**
```bash
# C++
cd eye
mkdir build-release && cd build-release
cmake .. -DCMAKE_BUILD_TYPE=Release
cmake --build . --config Release

# Rust
cargo build --release -p palmsync-brain
cargo build --release -p palmsync-daemon
```

**Yapılacaklar:**
- [ ] vcpkg.json oluştur (dependencies lockdown)
- [ ] CMake static linking konfigürasyonunu ayarla
- [ ] Rust release profile'ı optimize et
- [ ] Binary boyutlarını kontrol et (< 50MB ideal)
- [ ] Bağımlılık yok olduğunu doğrula (ldd/DependencyWalker)

---

### 2. Inno Setup Installer Scripti

**`installer/PalmSync.iss`:**

```ini
[Setup]
AppName=PalmSync
AppVersion=0.1.0
DefaultDirName={pf}\PalmSync
DefaultGroupName=PalmSync
OutputDir=dist
OutputBaseFilename=PalmSync-Setup

; Admin rights required for service installation
PrivilegesRequired=admin

[Files]
Source: "build\palmsync-eye.exe"; DestDir: "{app}"
Source: "build\palmsync-brain.exe"; DestDir: "{app}"
Source: "build\palmsync-daemon.exe"; DestDir: "{app}"
Source: "LICENSE"; DestDir: "{app}"
Source: "README.md"; DestDir: "{app}"

[Dirs]
Name: "{app}\logs"

[Icons]
Name: "{group}\PalmSync"; Filename: "{app}\palmsync-daemon.exe"
Name: "{group}\Uninstall PalmSync"; Filename: "{uninstallexe}"

[Run]
; Register service
Filename: "{app}\palmsync-daemon.exe"; Parameters: "install"; Flags: runhidden waituntilterminated

[UninstallRun]
; Unregister service
Filename: "{app}\palmsync-daemon.exe"; Parameters: "uninstall"; Flags: runhidden waituntilterminated

[Registry]
; Startup entry
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "PalmSync"; ValueType: string; ValueData: """{app}\palmsync-daemon.exe"""; Flags: uninsdeletevalue

; App info
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\PalmSync"; ValueName: "DisplayName"; ValueType: string; ValueData: "PalmSync"
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\PalmSync"; ValueName: "UninstallString"; ValueType: string; ValueData: "{uninstallexe}"
```

**Yapılacaklar:**
- [ ] Inno Setup 6.0+ yükle
- [ ] Build output klasörünü kur
- [ ] Registry entry'lerini yazı
- [ ] License file'ını include et
- [ ] Icon ve shortcut'ları ayarla
- [ ] Test installer

---

### 3. Daemon Service Yönetimi

**Rust'ta Service Manager implementasyonu:**

**`brain/src/service.rs` (opsiyonel - future use):**

```rust
#[cfg(target_os = "windows")]
pub mod windows_service {
    use std::ffi::OsString;
    use windows::Win32::System::Services::*;
    
    pub fn install_service(service_name: &str, exe_path: &str) -> anyhow::Result<()> {
        // TODO: windows-service crate'ini kullan
        // Service descriptor'ı kur
        // Auto-start flag'ını set et
        Ok(())
    }
    
    pub fn uninstall_service(service_name: &str) -> anyhow::Result<()> {
        // Service'i kaldır
        Ok(())
    }
    
    pub fn start_service(service_name: &str) -> anyhow::Result<()> {
        // Service'i başlat
        Ok(())
    }
}
```

**Alternatif: SC.exe Command-line (Installer'da):**

```batch
# Installation
sc create PalmSync displayname= "PalmSync Workspace Sync" 
sc config PalmSync start= auto
sc config PalmSync depend= Tcpip

# Start service
net start PalmSync

# Stop service
net stop PalmSync

# Uninstall
sc delete PalmSync
```

**`installer/install-service.bat`:**

```batch
@echo off
REM Admin check
net session >nul 2>&1
if %errorLevel% == 0 (
    echo Installing PalmSync service...
    sc create PalmSync displayname="PalmSync Workspace Synchronizer" binPath= "%~dp0palmsync-daemon.exe"
    sc config PalmSync start= auto
    net start PalmSync
    echo Service installed successfully.
) else (
    echo This script must be run as Administrator.
    exit /b 1
)
```

**Yapılacaklar:**
- [ ] Service registry entry'si oluştur
- [ ] Auto-start flag'ını set et
- [ ] Error handling'i implement et
- [ ] Service status'u check et
- [ ] Recovery action'ları kur (auto-restart)

---

### 4. Windows Service Integration (Advanced)

**`brain/Cargo.toml`'ye ekle:**

```toml
[dependencies]
windows-service = "0.6"
```

**`brain/src/windows_service.rs`:**

```rust
#[cfg(target_os = "windows")]
use windows_service::{
    define_windows_service,
    service::{ServiceControl, ServiceControlAccept, ServiceStatus, ServiceState, ServiceType},
    service_dispatcher,
};

#[cfg(target_os = "windows")]
define_windows_service!(ffi_service_main, my_service_main);

#[cfg(target_os = "windows")]
fn my_service_main(_args: Vec<OsString>) {
    if let Err(_) = run_service() {
        // log error
    }
}

#[cfg(target_os = "windows")]
fn run_service() -> Result<(), windows_service::Error> {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);

    let service_status_handle = service_control_handler::register(
        "PalmSync",
        move |control_event| -> ServiceControlAccept {
            match control_event {
                ServiceControl::Stop => {
                    let _ = shutdown_tx.blocking_send(());
                    ServiceControlAccept::STOP
                }
                ServiceControl::Interrogate => ServiceControlAccept::INTERROGATE,
                _ => ServiceControlAccept::NONE,
            }
        },
    )?;

    service_status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: windows_service::service::ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    // Tokio runtime'ını başlat ve daemon logic'i çalıştır
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                log::info!("Service shutdown signal received");
            }
            _ = crate::daemon::Daemon::new()?.run() => {
                log::info!("Daemon exited");
            }
        }
    });

    Ok(())
}
```

**Yapılacaklar:**
- [ ] windows-service crate'ini integrate et
- [ ] Service control handler'ı kur
- [ ] Start/Stop/Pause events'ini handle et
- [ ] Service status'u report et
- [ ] Graceful shutdown implement et

---

### 5. Registry Configuration

**Installation sırasında ayarlanacak registry entry'leri:**

```
HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\PalmSync
  - DisplayName: "PalmSync Workspace Synchronizer"
  - ImagePath: "C:\Program Files\PalmSync\palmsync-daemon.exe"
  - Start: 2 (Automatic)
  - Type: 16 (OwnProcess)
  - ErrorControl: 1 (Normal)
  - DelayedAutostart: 1 (Enable)

HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
  - PalmSync: "C:\Program Files\PalmSync\palmsync-daemon.exe"

HKEY_LOCAL_MACHINE\SOFTWARE\PalmSync
  - Version: "0.1.0"
  - InstallDir: "C:\Program Files\PalmSync"
  - LogDir: "C:\ProgramData\PalmSync\logs"
```

**Yapılacaklar:**
- [ ] Installer'da registry entry'leri yaz
- [ ] Uninstaller'da registry cleanup yap
- [ ] Version tracking'i implement et
- [ ] Config path'ını set et

---

### 6. Configuration File Management

**`C:\ProgramData\PalmSync\config.json`:**

```json
{
    "master_node": {
        "address": "127.0.0.1",
        "port": 9001,
        "password": "changeme"
    },
    "eye_service": {
        "enabled": true,
        "camera_id": 0,
        "fps": 30,
        "gesture_threshold": 0.3,
        "debounce_ms": 200
    },
    "brain_service": {
        "enabled": true,
        "ipc_port": 12345,
        "log_level": "info",
        "auto_connect_timeout_ms": 5000
    },
    "logging": {
        "enabled": true,
        "log_file": "C:\\ProgramData\\PalmSync\\logs\\palmsync.log",
        "max_size_mb": 50,
        "retention_days": 30
    }
}
```

**Yapılacaklar:**
- [ ] Config parser'ı implement et
- [ ] Default config'i oluştur
- [ ] Environment variable override'ını kur
- [ ] Config validation'ını implement et
- [ ] Hot reload support'u ekle (opsiyonel)

---

### 7. Logging Setup

**Kurulum sırasında log directory'si oluştur:**

```
C:\ProgramData\PalmSync\
├── logs/
│   ├── palmsync-eye.log
│   ├── palmsync-brain.log
│   └── palmsync-daemon.log
├── config.json
└── cache/
```

**Log Rotation implementasyonu:**

```rust
use log::LevelFilter;
use log4rs::Handle;
use log4rs::append::rolling_file::RollingFileAppender;

pub fn init_logging(log_dir: &str) -> anyhow::Result<Handle> {
    let appender = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S%.3f)} {l} {t}: {m}\n"
        )))
        .build(
            log_dir,
            Box::new(CompoundPolicy::new(
                Box::new(SizeTriggeringPolicy::new(50_000_000)), // 50MB
                Box::new(FixedWindowRollingPolicy::new(
                    // keep 10 files
                    "{}.{}",
                    10,
                    0,
                )),
            )),
        )?;

    Ok(Handle::default())
}
```

**Yapılacaklar:**
- [ ] Rolling file appender'ı kur
- [ ] Log size ve retention'ı limit et
- [ ] Log levels'i konfigür et
- [ ] Debug mode toggle'ını ekle

---

### 8. Toast Notifications

**`brain/src/notifications.rs` (Faz 5 için hazırlık):**

```rust
#[cfg(target_os = "windows")]
pub struct ToastNotification;

#[cfg(target_os = "windows")]
impl ToastNotification {
    pub fn show(title: &str, message: &str) -> anyhow::Result<()> {
        // Windows-rs ile native toast notification göster
        // Veya PowerShell aracılığıyla:
        let ps_cmd = format!(
            r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
            $APP_ID = 'PalmSync'
            [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($APP_ID).Show([Windows.UI.Notifications.ToastNotification]::new([xml]@"
<toast>
    <visual>
        <binding template="ToastText02">
            <text id="1">{}</text>
            <text id="2">{}</text>
        </binding>
    </visual>
</toast>
"@))"#,
            title, message
        );

        std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(ps_cmd)
            .output()?;

        Ok(())
    }
}
```

**Yapılacaklar:**
- [ ] Windows 10+ Toast Notification API'yi kul
- [ ] Fallback: Baloon tip (older Windows)
- [ ] Notification'ı async göster
- [ ] Service status events'inde notify et

---

### 9. Build Script Oluştur

**`build.ps1`:**

```powershell
param(
    [string]$Configuration = "Release"
)

Write-Host "Building PalmSync..." -ForegroundColor Green

# Check prerequisites
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "Rust/Cargo not found. Please install Rust from https://rustup.rs"
    exit 1
}

if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    Write-Error "CMake not found. Please install from https://cmake.org"
    exit 1
}

# Build C++ (Eye)
Write-Host "Building Eye (C++)..." -ForegroundColor Cyan
$eyeBuildDir = "eye\build-release"
if (Test-Path $eyeBuildDir) { Remove-Item $eyeBuildDir -Recurse -Force }
New-Item -ItemType Directory $eyeBuildDir | Out-Null

Push-Location $eyeBuildDir
cmake .. -DCMAKE_BUILD_TYPE=Release
cmake --build . --config Release
Pop-Location

# Build Rust (Brain & Daemon)
Write-Host "Building Brain (Rust)..." -ForegroundColor Cyan
cargo build --release -p palmsync-brain

Write-Host "Building Daemon (Rust)..." -ForegroundColor Cyan
cargo build --release -p palmsync-daemon

# Create installer build folder
Write-Host "Preparing installer..." -ForegroundColor Cyan
$buildDir = "build"
if (Test-Path $buildDir) { Remove-Item $buildDir -Recurse -Force }
New-Item -ItemType Directory $buildDir | Out-Null

Copy-Item "eye\build-release\Release\palmsync-eye.exe" $buildDir -Force
Copy-Item "target\release\palmsync-brain.exe" $buildDir -Force
Copy-Item "target\release\palmsync-daemon.exe" $buildDir -Force
Copy-Item "LICENSE" $buildDir -Force
Copy-Item "README.md" $buildDir -Force

Write-Host "Build complete! Installer ready at: $buildDir" -ForegroundColor Green
```

**Çalıştır:**
```powershell
.\build.ps1
```

**Yapılacaklar:**
- [ ] PowerShell build script'i yaz
- [ ] Prerequisite check'ler ekle
- [ ] Output validation'ı implement et
- [ ] Error handling'i kur
- [ ] Cross-compilation support'u ekle (opsiyonel)

---

### 10. Testing ve Validation

**`scripts/test-installer.ps1`:**

```powershell
Write-Host "Testing installer..." -ForegroundColor Green

# Prerequisite checks
$checks = @(
    @{
        Name = "Rust installed"
        Command = "rustc --version"
    },
    @{
        Name = "CMake installed"
        Command = "cmake --version"
    },
    @{
        Name = "Inno Setup installed"
        Command = "iscc /?"
    }
)

foreach ($check in $checks) {
    try {
        & $check.Command | Out-Null
        Write-Host "✓ $($check.Name)" -ForegroundColor Green
    } catch {
        Write-Host "✗ $($check.Name)" -ForegroundColor Red
        exit 1
    }
}

# Build
.\build.ps1

# Create installer
Write-Host "Creating installer..." -ForegroundColor Cyan
iscc installer\PalmSync.iss

# Verify installer
if (Test-Path "dist\PalmSync-Setup.exe") {
    Write-Host "✓ Installer created: dist\PalmSync-Setup.exe" -ForegroundColor Green
} else {
    Write-Host "✗ Installer creation failed" -ForegroundColor Red
    exit 1
}

# Size check
$size = (Get-Item "dist\PalmSync-Setup.exe").Length / 1MB
Write-Host "Installer size: $([Math]::Round($size, 2)) MB"

Write-Host "Installer testing complete!" -ForegroundColor Green
```

**Yapılacaklar:**
- [ ] Prerequisite check script'i yaz
- [ ] Installer creation'ını test et
- [ ] File integrity'sini doğrula
- [ ] Registry entry'lerini verify et
- [ ] Service auto-start'ını kontrol et
- [ ] Uninstall process'ini test et

---

## 📦 Deliverables

Bu faz tamamlandığında:

1. ✅ **Statik Binary'ler**: eye.exe, brain.exe, daemon.exe
2. ✅ **Inno Setup Installer**: Single-click installation
3. ✅ **Service Registration**: Auto-start Windows Service
4. ✅ **Registry Configuration**: Startup entries
5. ✅ **Log Directory**: ProgramData yapısı
6. ✅ **Uninstaller**: Clean removal
7. ✅ **Build Script**: Automated compilation

---

## 🔄 Sonraki Faz Bağlantıları

- **Faz 5 (UI)**: System tray app ve advanced notifications
- **Faz 6 (Security)**: Installer signing ve digital certificate

---

## 📝 Uyarılar ve Best Practices

⚠️ **Deployment Consideration'ları:**

1. **Statik Linking Challenges:**
   - OpenCV CRT linking'i double-check et
   - vcpkg kullanırken triplet'i doğru set et
   - Deployment target'ında test et

2. **Service Permission'ları:**
   - Eye service'in camera erişimi olmalı
   - Brain service'in VS Code erişimi olmalı
   - Admin rights gereksiz ise remove et

3. **Antivirus Considerations:**
   - Installer'ı VirusTotal'de scan et
   - Unsigned binary'ler flagged olabilir (Faz 6'da fix et)
   - User approval'ı almaya hazırlıkta ol

4. **Update Mechanism:**
   - Installer versioning'i kur (future updates için)
   - Config migration strategy'i hazırla
   - Rollback capability'i düşün

5. **Windows Compatibility:**
   - Windows 10 21H2+ minimum
   - Server editions'ında test et
   - Locale-specific issue'leri handle et

🔐 **Güvenlik Kontrol Listesi:**
- [ ] Binary'lerin hardcoded secret'ları yok
- [ ] Registry entry'leri trusted yerde
- [ ] Service account minimal permissions'la çalışır
- [ ] Logs'ta sensitive data yok
- [ ] UAC bypass yok

---

**Son Güncelleme**: 2026-07-20  
**Statüsü**: 🔵 Hazır Implementation için
