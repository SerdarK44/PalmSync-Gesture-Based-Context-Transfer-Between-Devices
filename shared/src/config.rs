use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub node_id: String,
    /// "master" or "slave". Determines how the daemon starts when no
    /// CLI mode argument is given (i.e. when run as a Windows service).
    #[serde(default = "default_role")]
    pub role: String,
    pub master_addr: String,
    pub master_port: u16,
    pub password: String,
    pub ipc_port: u16,
    pub log_level: String,
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
    #[serde(default = "default_log_max_size_mb")]
    pub log_max_size_mb: u64,
    #[serde(default)]
    pub enable_notifications: bool,
    /// Verbose logging everywhere + full system-check reports to the host.
    #[serde(default)]
    pub debug_mode: bool,
    /// User-visible data folder (Documents\PalmSync): received files live
    /// in its "Gelen Dosyalar" subfolder. Written by the UI, which knows
    /// where Documents really is (it may be redirected to OneDrive).
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

fn default_data_dir() -> String {
    std::env::var("USERPROFILE")
        .map(|h| PathBuf::from(h).join("Documents").join("PalmSync").to_string_lossy().into_owned())
        .unwrap_or_else(|_| "PalmSync".into())
}

fn default_role() -> String {
    "slave".into()
}

fn default_log_dir() -> String {
    config_dir()
        .map(|d| d.join("logs").to_string_lossy().into_owned())
        .unwrap_or_else(|| "logs".into())
}

fn default_log_max_size_mb() -> u64 {
    10
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            role: default_role(),
            master_addr: "127.0.0.1".into(),
            master_port: 9001,
            password: "palmsync".into(),
            ipc_port: 12345,
            log_level: "info".into(),
            log_dir: default_log_dir(),
            log_max_size_mb: 10,
            enable_notifications: true,
            debug_mode: false,
            data_dir: default_data_dir(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let mut config = Self::load_from_file().unwrap_or_default();
        config.apply_env_overrides();
        config
    }

    pub fn from_env() -> Self {
        Self::load()
    }

    fn load_from_file() -> Option<Self> {
        let path = config_file_path()?;
        let data = std::fs::read_to_string(&path).ok()?;
        let cfg: Config = serde_json::from_str(&data).ok()?;
        log::debug!("Config loaded from {}", path.display());
        Some(cfg)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(role) = std::env::var("PALMSYNC_ROLE") {
            self.role = role;
        }
        if let Ok(addr) = std::env::var("PALMSYNC_MASTER_ADDR") {
            self.master_addr = addr;
        }
        if let Ok(port) = std::env::var("PALMSYNC_MASTER_PORT") {
            if let Ok(p) = port.parse() {
                self.master_port = p;
            }
        }
        if let Ok(pass) = std::env::var("PALMSYNC_PASSWORD") {
            self.password = pass;
        }
        if let Ok(ipc) = std::env::var("PALMSYNC_IPC_PORT") {
            if let Ok(p) = ipc.parse() {
                self.ipc_port = p;
            }
        }
        if let Ok(level) = std::env::var("PALMSYNC_LOG_LEVEL") {
            self.log_level = level;
        }
    }

    pub fn save_default_config() -> anyhow::Result<PathBuf> {
        let path = config_file_path()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine config directory"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let config = Self::default();
        let json = serde_json::to_string_pretty(&config)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    pub fn ws_master_url(&self) -> String {
        format!("ws://{}:{}", self.master_addr, self.master_port)
    }

    /// Where a master binds its listener. Always all interfaces so slaves
    /// on other machines can reach it — `master_addr` is only the address
    /// slaves dial, not a bind restriction.
    pub fn bind_addr(&self) -> String {
        format!("0.0.0.0:{}", self.master_port)
    }

    /// Human-readable listen address for logs.
    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.master_addr, self.master_port)
    }
}

pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // Per-user LocalAppData so the daemon runs in the user's session
        // (required to see their VS Code window + clipboard) and can read/write
        // config without administrator rights.
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|pd| PathBuf::from(pd).join("PalmSync"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Some(PathBuf::from("/etc/palmsync"))
    }
}

pub fn config_file_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}
