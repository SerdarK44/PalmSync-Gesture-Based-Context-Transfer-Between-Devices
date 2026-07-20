use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static AUDIT_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Set the audit log destination once at startup (e.g. <log_dir>/audit.log).
/// Until initialized, events fall back to the regular logger only.
pub fn init(log_dir: &str) {
    let path = PathBuf::from(log_dir).join("audit.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    *AUDIT_PATH.lock().unwrap() = Some(path);
}

pub fn log_auth(node_id: &str, addr: &str, success: bool) {
    write_entry(&format!(
        "[AUTH] node_id={} addr={} success={}",
        node_id, addr, success
    ));
}

pub fn log_sync(node_id: &str, operation: &str, detail: &str) {
    write_entry(&format!(
        "[SYNC] node_id={} operation={} detail={}",
        node_id, operation, detail
    ));
}

pub fn log_security(event: &str, details: &str) {
    write_entry(&format!("[SECURITY] event={} details={}", event, details));
}

fn write_entry(entry: &str) {
    let now = chrono::Local::now().to_rfc3339();
    let line = format!("{} {}", now, entry);

    log::info!(target: "audit", "{}", entry);

    let path = AUDIT_PATH.lock().unwrap().clone();
    if let Some(path) = path {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{}", line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_writes_to_file() {
        let dir = std::env::temp_dir().join("palmsync-audit-test");
        let _ = std::fs::remove_dir_all(&dir);
        init(dir.to_str().unwrap());

        log_auth("node-x", "127.0.0.1:5555", true);
        log_security("rate_limit", "ip=10.0.0.1");

        let content = std::fs::read_to_string(dir.join("audit.log")).unwrap();
        assert!(content.contains("[AUTH] node_id=node-x"));
        assert!(content.contains("[SECURITY] event=rate_limit"));
    }
}
