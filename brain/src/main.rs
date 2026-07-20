#![windows_subsystem = "windows"]

mod clipboard;
mod context_capture;
mod daemon;
mod executor;
mod ipc_listener;
mod logging;
mod notifications;
mod service_ctl;

use anyhow::Result;
use palmsync_shared::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load();

    logging::init(&config.log_dir, &config.log_level)
        .unwrap_or_else(|e| {
            eprintln!("Logging init failed ({}), falling back to stderr", e);
            env_logger::Builder::from_env(
                env_logger::Env::default().default_filter_or(&config.log_level),
            )
            .format_timestamp_millis()
            .init();
        });

    palmsync_shared::audit::init(&config.log_dir);

    log::info!(
        "PalmSync Brain v{} starting — node {}",
        env!("CARGO_PKG_VERSION"),
        &config.node_id[..8]
    );

    // CLI arg wins (master/slave/install/uninstall); otherwise fall back to
    // config.role so the Windows service picks the right role per machine.
    let cli_mode = std::env::args().nth(1).unwrap_or_default();
    let mode = match cli_mode.as_str() {
        "" => config.role.clone(),
        other => other.to_string(),
    };

    match mode.as_str() {
        "master" => {
            log::info!("Starting in MASTER mode on {}", config.listen_addr());
            daemon::run_master(config).await
        }
        "install" => {
            log::info!("Installing PalmSync service...");
            service_ctl::install()?;
            log::info!("Service installed successfully");
            Ok(())
        }
        "uninstall" => {
            log::info!("Uninstalling PalmSync service...");
            service_ctl::uninstall()?;
            log::info!("Service uninstalled successfully");
            Ok(())
        }
        _ => {
            log::info!(
                "Starting in SLAVE mode, master at {}",
                config.ws_master_url()
            );
            daemon::run_slave(config).await
        }
    }
}
