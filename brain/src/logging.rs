use std::fs;
use std::path::Path;

pub fn init(log_dir: &str, level: &str) -> anyhow::Result<()> {
    let log_dir = Path::new(log_dir);
    fs::create_dir_all(log_dir)?;

    let log_file = log_dir.join("palmsync-brain.log");

    let level_filter = match level {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(level_filter)
        .chain(std::io::stderr())
        .chain(fern::log_file(&log_file)?)
        .apply()?;

    log::info!("Logging initialized -> {}", log_file.display());
    Ok(())
}
