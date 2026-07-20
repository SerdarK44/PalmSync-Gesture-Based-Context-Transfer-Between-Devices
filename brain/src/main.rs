#![windows_subsystem = "windows"]

mod clipboard;
mod context_capture;
mod daemon;
mod executor;
mod ipc_listener;

use anyhow::Result;
use palmsync_shared::Config;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let config = Config::from_env();
    log::info!("PalmSync Brain starting — node {}", &config.node_id[..8]);

    let mode = std::env::args().nth(1).unwrap_or_default();

    match mode.as_str() {
        "master" => {
            log::info!("Starting in MASTER mode on {}", config.listen_addr());
            daemon::run_master(config).await
        }
        _ => {
            log::info!("Starting in SLAVE mode, master at {}", config.ws_master_url());
            daemon::run_slave(config).await
        }
    }
}
