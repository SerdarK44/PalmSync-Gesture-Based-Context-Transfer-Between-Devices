use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use palmsync_networking::DEVICE_REPORT_TARGET;
use tokio::sync::mpsc;

/// How many rotated files to keep next to the live log (`.1` is the newest).
const KEEP_ROTATED: usize = 3;

/// A WARN/ERROR line this daemon logged, waiting to be forwarded to the host.
pub struct LocalProblem {
    pub level: log::Level,
    pub message: String,
}

/// Set (on a joined machine) to receive this daemon's own warnings/errors so
/// they can be forwarded to the host. Unset = nothing is forwarded.
static PROBLEM_SINK: OnceLock<mpsc::UnboundedSender<LocalProblem>> = OnceLock::new();

/// Start capturing this daemon's WARN/ERROR lines (see [`PROBLEM_SINK`]).
pub fn capture_problems() -> mpsc::UnboundedReceiver<LocalProblem> {
    let (tx, rx) = mpsc::unbounded_channel();
    let _ = PROBLEM_SINK.set(tx);
    rx
}

/// Line format shared by the daemon, the camera (Eye) and the UI so the log
/// viewer can merge them:  `2026-10-11 14:08:47.123 [INFO] target: message`
///
/// Files in `log_dir`:
/// - `palmsync-brain.log`   this daemon
/// - `cihaz-raporlari.log`  problems reported by every machine in the session
///   (filled on the host)
pub fn init(log_dir: &str, level: &str, max_size_mb: u64, debug_mode: bool) -> anyhow::Result<()> {
    let log_dir = Path::new(log_dir);
    fs::create_dir_all(log_dir)?;

    let log_file = log_dir.join("palmsync-brain.log");
    let max_bytes = max_size_mb.clamp(1, 100) * 1024 * 1024;
    let writer = RotatingFile::open(log_file.clone(), max_bytes)?;
    let reports = RotatingFile::open(log_dir.join("cihaz-raporlari.log"), max_bytes)?;

    let level_filter = if debug_mode {
        log::LevelFilter::Debug
    } else {
        match level {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        }
    };

    let is_report = |m: &log::Metadata| m.target() == DEVICE_REPORT_TARGET;

    fern::Dispatch::new()
        .format(|out, message, record| {
            // One entry = one line: indent continuation lines so the viewer
            // can still tell where an entry starts.
            let msg = message.to_string().replace('\n', "\n    ");
            out.finish(format_args!(
                "{} [{}] {}: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.target(),
                msg
            ))
        })
        .level(level_filter)
        // Device reports are always kept, whatever the configured level.
        .level_for(DEVICE_REPORT_TARGET, log::LevelFilter::Info)
        // Third-party crates are extremely chatty at debug/trace level and
        // drown out PalmSync's own messages.
        .level_for("tungstenite", log::LevelFilter::Warn)
        .level_for("tokio_tungstenite", log::LevelFilter::Warn)
        .level_for("mio", log::LevelFilter::Warn)
        .chain(std::io::stderr())
        .chain(
            fern::Dispatch::new()
                .filter(move |m| !is_report(m))
                .chain(Box::new(writer) as Box<dyn Write + Send>),
        )
        .chain(
            fern::Dispatch::new()
                .filter(move |m| is_report(m))
                .chain(Box::new(reports) as Box<dyn Write + Send>),
        )
        .chain(
            fern::Dispatch::new()
                .level(log::LevelFilter::Warn)
                // Never forward the link's own errors (a broken link would
                // only generate more reports about itself).
                .filter(move |m| !is_report(m) && !m.target().starts_with("palmsync_networking"))
                .chain(fern::Output::call(|record| {
                    if let Some(tx) = PROBLEM_SINK.get() {
                        // Children receive the already-formatted line
                        // ("<time> [WARN] target: msg"): keep only msg.
                        let line = record.args().to_string();
                        let marker = format!("{}: ", record.target());
                        let message = match line.find(&marker) {
                            Some(i) => line[i + marker.len()..].to_string(),
                            None => line,
                        };
                        let _ = tx.send(LocalProblem {
                            level: record.level(),
                            message,
                        });
                    }
                })),
        )
        .apply()?;

    log::info!("==================== PalmSync daemon started ====================");
    log::info!(
        "Logging to {} (level {}, rotates at {} MB{})",
        log_file.display(),
        level_filter,
        max_size_mb,
        if debug_mode {
            ", HATA AYIKLAMA MODU"
        } else {
            ""
        }
    );
    Ok(())
}

/// Append-only log file that rotates itself when it grows past `max_bytes`:
/// `x.log` -> `x.log.1` -> `x.log.2` ... (oldest beyond KEEP_ROTATED deleted).
/// Previously the log grew forever.
struct RotatingFile {
    path: PathBuf,
    file: File,
    written: u64,
    max_bytes: u64,
}

impl RotatingFile {
    fn open(path: PathBuf, max_bytes: u64) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut me = Self {
            path,
            file,
            written,
            max_bytes,
        };
        if me.written >= me.max_bytes {
            me.rotate()?;
        }
        Ok(me)
    }

    fn rotated(&self, n: usize) -> PathBuf {
        let mut s = self.path.clone().into_os_string();
        s.push(format!(".{}", n));
        PathBuf::from(s)
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let _ = fs::remove_file(self.rotated(KEEP_ROTATED));
        for n in (1..KEEP_ROTATED).rev() {
            let _ = fs::rename(self.rotated(n), self.rotated(n + 1));
        }
        // Rename can fail if another process holds the file without
        // FILE_SHARE_DELETE; then just truncate so the size stays bounded.
        if fs::rename(&self.path, self.rotated(1)).is_err() {
            self.file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&self.path)?;
        }
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.written = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written + buf.len() as u64 > self.max_bytes {
            // Never lose the line just because rotation failed.
            let _ = self.rotate();
        }
        let n = self.file.write(buf)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_and_keeps_bounded_history() {
        let dir = std::env::temp_dir().join(format!("palmsync-logrot-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.log");

        let mut w = RotatingFile::open(path.clone(), 100).unwrap();
        for i in 0..50 {
            writeln!(w, "line {:03} ......................", i).unwrap();
        }
        w.flush().unwrap();

        assert!(fs::metadata(&path).unwrap().len() <= 100);
        assert!(w.rotated(1).exists());
        assert!(
            !w.rotated(KEEP_ROTATED + 1).exists(),
            "old files must be pruned"
        );
        let newest = fs::read_to_string(&path).unwrap();
        assert!(
            newest.contains("line 049"),
            "latest line must be in the live file"
        );

        drop(w);
        fs::remove_dir_all(&dir).ok();
    }
}
