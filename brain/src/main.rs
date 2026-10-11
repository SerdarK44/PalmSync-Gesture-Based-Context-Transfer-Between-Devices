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
    // Answer --version before touching the logs: the UI's system check runs
    // it to test that the exe can start, and must not add "daemon started"
    // lines (which also reset the UI's connection-status tracking).
    if matches!(std::env::args().nth(1).as_deref(), Some("--version" | "-v")) {
        println!("palmsync-daemon {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let config = Config::load();

    logging::init(&config.log_dir, &config.log_level, config.log_max_size_mb, config.debug_mode)
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
        palmsync_shared::short(&config.node_id, 8)
    );

    // CLI arg wins (master/slave/install/uninstall/help); otherwise fall back to
    // config.role so the daemon picks the right role per machine.
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
        "slave" => {
            log::info!(
                "Starting in SLAVE mode, master at {}",
                config.ws_master_url()
            );
            daemon::run_slave(config).await
        }
        "off" => {
            log::info!("PalmSync daemon is idle (role='off').");
            daemon::run_idle(config).await
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
        "--help" | "-h" | "help" => {
            println!("PalmSync Daemon v{}", env!("CARGO_PKG_VERSION"));
            println!("\nUsage: palmsync-daemon [master|slave|off|install|uninstall|help]");
            println!("\nCommands:");
            println!("  master     Run in master mode");
            println!("  slave      Run in slave mode");
            println!("  off        Run in idle mode (waits for UI session activation)");
            println!("  install    Install Windows service");
            println!("  uninstall  Uninstall Windows service");
            println!("  --help     Show this help");
            println!("  --version  Show version");
            Ok(())
        }
        "--version" | "-v" => {
            println!("palmsync-daemon {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => {
            anyhow::bail!("Unknown daemon mode: '{}'. Use --help for usage.", other);
        }
    }
}
