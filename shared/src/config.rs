use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub node_id: String,
    pub master_addr: String,
    pub master_port: u16,
    pub password: String,
    pub ipc_port: u16,
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            master_addr: "127.0.0.1".into(),
            master_port: 9001,
            password: "palmsync".into(),
            ipc_port: 12345,
            log_level: "info".into(),
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(addr) = std::env::var("PALMSYNC_MASTER_ADDR") {
            config.master_addr = addr;
        }
        if let Ok(port) = std::env::var("PALMSYNC_MASTER_PORT") {
            if let Ok(p) = port.parse() {
                config.master_port = p;
            }
        }
        if let Ok(pass) = std::env::var("PALMSYNC_PASSWORD") {
            config.password = pass;
        }
        if let Ok(ipc) = std::env::var("PALMSYNC_IPC_PORT") {
            if let Ok(p) = ipc.parse() {
                config.ipc_port = p;
            }
        }
        if let Ok(level) = std::env::var("PALMSYNC_LOG_LEVEL") {
            config.log_level = level;
        }

        config
    }

    pub fn ws_master_url(&self) -> String {
        format!("ws://{}:{}", self.master_addr, self.master_port)
    }

    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.master_addr, self.master_port)
    }
}
