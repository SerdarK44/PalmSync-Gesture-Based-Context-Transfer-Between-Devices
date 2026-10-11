#![windows_subsystem = "windows"]

mod apps;
mod clipboard;
mod context_capture;
mod daemon;
mod executor;
mod ipc_listener;
mod logging;
mod notifications;
mod win;

use anyhow::Result;
use palmsync_shared::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Answer --version before touching the logs: the UI's system check runs
    // it to test that the exe can start, and must not add "daemon started"
    // lines (which also reset the UI's connection-status tracking).
    if matches!(std::env::args().nth(1).as_deref(), Some("--version" | "-v")) {
        println!("palmsync-daemon {}", env!("PALMSYNC_VERSION"));
        return Ok(());
    }

    // Window screenshots need real pixel sizes on scaled (125%+) displays.
    win::set_dpi_aware();

    let config = Config::load();

    logging::init(
        &config.log_dir,
        &config.log_level,
        config.log_max_size_mb,
        config.debug_mode,
    )
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
        env!("PALMSYNC_VERSION"),
        palmsync_shared::short(&config.node_id, 8)
    );

    // CLI arg wins (master/slave/off/help); otherwise fall back to
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
        "capture-test" => {
            // Diagnostics: what would a GRAB send right now? Prints a summary
            // (sizes, not contents) of the window in front after 3 seconds.
            println!("Switch to the window to test — capturing in 3 s...");
            std::thread::sleep(std::time::Duration::from_secs(3));
            let snap = context_capture::capture_context("capture-test")?;
            println!("summary   : {}", context_capture::describe(&snap));
            println!("app       : {:?}", snap.app);
            println!("file      : {} (text {} B, binary {} B base64)", snap.file_name,
                     snap.file_content.len(), snap.file_data_b64.len());
            println!("url       : {}", snap.browser_url);
            println!("clipboard : {} B", snap.clipboard_content.len());
            println!("screenshot: {} B base64", snap.screenshot_jpeg_b64.len());
            palmsync_shared::validation::Validator::validate_snapshot(&snap)?;
            println!("valid     : yes");
            if let Some(out) = std::env::args().nth(2) {
                std::fs::write(&out, serde_json::to_string_pretty(&snap)?)?;
                println!("saved     : {}", out);
            }
            Ok(())
        }
        "restore-test" => {
            // Diagnostics: restore a snapshot saved by `capture-test <file>`
            // on THIS machine, exactly as a DROP would.
            let path = std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("usage: palmsync-daemon restore-test <snapshot.json>"))?;
            let snap: palmsync_shared::StateSnapshot =
                serde_json::from_str(std::fs::read_to_string(path)?.trim_start_matches('\u{feff}'))?;
            palmsync_shared::validation::Validator::validate_snapshot(&snap)?;
            println!("{}", executor::restore(&snap));
            Ok(())
        }
        "--help" | "-h" | "help" => {
            println!("PalmSync Daemon v{}", env!("PALMSYNC_VERSION"));
            println!("\nUsage: palmsync-daemon [master|slave|off|capture-test|restore-test|help]");
            println!("\nCommands:");
            println!("  master     Run in master mode");
            println!("  slave      Run in slave mode");
            println!("  off        Run in idle mode (waits for UI session activation)");
            println!("  capture-test [out.json]   Show (and save) what a GRAB would capture from the window in front");
            println!("  restore-test <in.json>    Restore a saved snapshot here, as a DROP would");
            println!("  --help     Show this help");
            println!("  --version  Show version");
            Ok(())
        }
        "--version" | "-v" => {
            println!("palmsync-daemon {}", env!("PALMSYNC_VERSION"));
            Ok(())
        }
        other => {
            anyhow::bail!("Unknown daemon mode: '{}'. Use --help for usage.", other);
        }
    }
}
