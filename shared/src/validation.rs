use crate::{DeviceReport, StateSnapshot};

pub const MAX_PATH_LEN: usize = 4096;
pub const MAX_FILE_NAME_LEN: usize = 512;
pub const MAX_CLIPBOARD_SIZE: usize = 10 * 1024 * 1024; // 10 MB
pub const MAX_NODE_ID_LEN: usize = 128;
/// Largest file whose contents travel with a snapshot. Bigger files are sent
/// as name/path only. (The E2EE envelope hex-encodes, so the wire size is ~2x.)
pub const MAX_FILE_CONTENT_SIZE: usize = 2 * 1024 * 1024; // 2 MB
/// Largest NON-text document that travels with a snapshot (docx, pdf, png...).
pub const MAX_FILE_DATA_SIZE: usize = 8 * 1024 * 1024; // 8 MB raw
/// Base64 length of the above.
pub const MAX_FILE_DATA_B64_LEN: usize = MAX_FILE_DATA_SIZE.div_ceil(3) * 4;
/// Window screenshot (base64 JPEG).
pub const MAX_SCREENSHOT_B64_LEN: usize = 2 * 1024 * 1024;
pub const MAX_APP_PROCESS_LEN: usize = 64;
pub const MAX_APP_TEXT_LEN: usize = 512;
pub const APP_KINDS: &[&str] = &["", "browser", "editor", "document", "files", "other"];
pub const MAX_REPORT_MESSAGE_LEN: usize = 2000;
pub const MAX_REPORT_MACHINE_LEN: usize = 64;
pub const MAX_REPORT_SOURCE_LEN: usize = 32;

pub struct Validator;

impl Validator {
    /// Validate a workspace path received from the network.
    /// Absolute paths are expected (VS Code projects), but traversal
    /// sequences, control characters and NUL bytes are rejected.
    pub fn validate_path(path: &str) -> anyhow::Result<()> {
        if path.is_empty() {
            anyhow::bail!("path is empty");
        }
        if path.len() > MAX_PATH_LEN {
            anyhow::bail!("path too long ({} > {})", path.len(), MAX_PATH_LEN);
        }
        if path.contains("..") {
            anyhow::bail!("path contains traversal sequence '..'");
        }
        if path.chars().any(|c| c.is_control()) {
            anyhow::bail!("path contains control characters");
        }
        Ok(())
    }

    /// Validate a bare file name (no separators allowed).
    pub fn validate_file_name(name: &str) -> anyhow::Result<()> {
        if name.is_empty() {
            anyhow::bail!("file name is empty");
        }
        if name.len() > MAX_FILE_NAME_LEN {
            anyhow::bail!("file name too long");
        }
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            anyhow::bail!("file name contains path separators");
        }
        if name.chars().any(|c| c.is_control()) {
            anyhow::bail!("file name contains control characters");
        }
        Ok(())
    }

    /// Validate a path *relative* to the project folder (e.g. `src/main.rs`).
    /// The receiver writes the file under its own folder using this path, so
    /// it must never escape it: no `..`, no absolute paths, no drive letters.
    pub fn validate_rel_path(path: &str) -> anyhow::Result<()> {
        if path.len() > MAX_PATH_LEN {
            anyhow::bail!("relative path too long");
        }
        if path.starts_with('/') || path.starts_with('\\') || path.contains(':') {
            anyhow::bail!("relative path must not be absolute");
        }
        if path.chars().any(|c| c.is_control()) {
            anyhow::bail!("relative path contains control characters");
        }
        for part in path.split(['/', '\\']) {
            if part.is_empty() || part == "." || part == ".." {
                anyhow::bail!("relative path has an invalid component");
            }
        }
        Ok(())
    }

    pub fn validate_ip_address(ip: &str) -> anyhow::Result<()> {
        ip.parse::<std::net::IpAddr>()
            .map(|_| ())
            .map_err(|_| anyhow::anyhow!("invalid IP address: {}", ip))
    }

    pub fn validate_port(port: u16) -> anyhow::Result<()> {
        if port < 1024 {
            anyhow::bail!("port {} < 1024 requires elevated privileges", port);
        }
        Ok(())
    }

    pub fn validate_node_id(node_id: &str) -> anyhow::Result<()> {
        if node_id.is_empty() || node_id.len() > MAX_NODE_ID_LEN {
            anyhow::bail!("invalid node_id length");
        }
        if !node_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!("node_id contains invalid characters");
        }
        Ok(())
    }

    /// Validate a full snapshot received over the network before acting on it.
    ///
    /// `project_path` and `file_name` are OPTIONAL: a snapshot may carry only a
    /// browser tab or only the clipboard (nothing was open in VS Code). They are
    /// validated only when present. A snapshot must still carry *something*.
    pub fn validate_snapshot(snap: &StateSnapshot) -> anyhow::Result<()> {
        if !snap.project_path.is_empty() {
            Self::validate_path(&snap.project_path)?;
        }
        if !snap.file_name.is_empty() {
            Self::validate_file_name(&snap.file_name)?;
        }
        Self::validate_node_id(&snap.source_node_id)?;

        Self::validate_app(&snap.app)?;

        if snap.file_name.is_empty()
            && snap.browser_url.is_empty()
            && snap.clipboard_content.is_empty()
            && snap.app.process.is_empty()
            && snap.screenshot_jpeg_b64.is_empty()
        {
            anyhow::bail!("snapshot carries no content (no file, url, clipboard or app)");
        }
        if snap.file_data_b64.len() > MAX_FILE_DATA_B64_LEN {
            anyhow::bail!("file data too large ({} base64 bytes)", snap.file_data_b64.len());
        }
        if !snap.file_data_b64.is_empty() {
            if snap.file_name.is_empty() {
                anyhow::bail!("file data without a file name");
            }
            if !snap.file_content.is_empty() {
                anyhow::bail!("both text and binary file contents set");
            }
            Self::validate_base64(&snap.file_data_b64)?;
        }
        if snap.screenshot_jpeg_b64.len() > MAX_SCREENSHOT_B64_LEN {
            anyhow::bail!("screenshot too large");
        }
        if !snap.screenshot_jpeg_b64.is_empty() {
            Self::validate_base64(&snap.screenshot_jpeg_b64)?;
        }
        if snap.clipboard_content.len() > MAX_CLIPBOARD_SIZE {
            anyhow::bail!(
                "clipboard content too large ({} bytes)",
                snap.clipboard_content.len()
            );
        }
        if !snap.file_rel_path.is_empty() {
            Self::validate_rel_path(&snap.file_rel_path)?;
        }
        if snap.file_content.len() > MAX_FILE_CONTENT_SIZE {
            anyhow::bail!("file content too large ({} bytes)", snap.file_content.len());
        }
        if !snap.file_content.is_empty() && snap.file_name.is_empty() {
            anyhow::bail!("file content without a file name");
        }
        if snap.cursor_line > 10_000_000 {
            anyhow::bail!("cursor line out of range");
        }
        if !snap.browser_url.is_empty() {
            if snap.browser_url.len() > 2048 {
                anyhow::bail!("browser url too long");
            }
            if !(snap.browser_url.starts_with("http://")
                || snap.browser_url.starts_with("https://"))
            {
                anyhow::bail!("browser url must be http(s)");
            }
            if snap.browser_url.chars().any(|c| c.is_control()) {
                anyhow::bail!("browser url contains control characters");
            }
        }
        Ok(())
    }

    /// The receiver only ever uses these to LOOK UP apps and files on its
    /// own disk (never executes a path from the network), but they still
    /// end up in logs and paths, so keep them small and printable.
    pub fn validate_app(app: &crate::AppContext) -> anyhow::Result<()> {
        if app.process.len() > MAX_APP_PROCESS_LEN
            || !app
                .process
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._+- ".contains(c))
        {
            anyhow::bail!("invalid app process name");
        }
        if !APP_KINDS.contains(&app.kind.as_str()) {
            anyhow::bail!("invalid app kind");
        }
        for (name, value) in [
            ("display name", &app.display_name),
            ("window title", &app.window_title),
        ] {
            if value.len() > MAX_APP_TEXT_LEN || value.chars().any(|c| c.is_control()) {
                anyhow::bail!("invalid app {}", name);
            }
        }
        for (name, value) in [
            ("exe path", &app.exe_path),
            ("document path", &app.document_path),
            ("folder path", &app.folder_path),
        ] {
            if !value.is_empty() {
                Self::validate_path(value).map_err(|e| anyhow::anyhow!("app {}: {}", name, e))?;
            }
        }
        Ok(())
    }

    /// Standard base64 alphabet with padding only — rejects anything that is
    /// not what the sender's encoder produces.
    pub fn validate_base64(data: &str) -> anyhow::Result<()> {
        let body = data.trim_end_matches('=');
        if !data.len().is_multiple_of(4)
            || data.len() - body.len() > 2
            || !body
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
        {
            anyhow::bail!("invalid base64 data");
        }
        Ok(())
    }

    /// Validate a device report received from the network and return a
    /// sanitized copy safe to write into the host's log (no control chars
    /// that could forge extra log lines, bounded sizes).
    pub fn sanitize_report(r: &DeviceReport) -> anyhow::Result<DeviceReport> {
        if !matches!(r.level.as_str(), "error" | "warn" | "info") {
            anyhow::bail!("invalid report level");
        }
        if r.source.is_empty()
            || r.source.len() > MAX_REPORT_SOURCE_LEN
            || !r
                .source
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!("invalid report source");
        }
        if r.message.trim().is_empty() {
            anyhow::bail!("empty report message");
        }
        let clean = |s: &str, max: usize| -> String {
            let one_line: String = s
                .chars()
                .map(|c| {
                    if c == '\n' || c == '\r' || c == '\t' {
                        ' '
                    } else {
                        c
                    }
                })
                .filter(|c| !c.is_control())
                .collect();
            one_line
                .chars()
                .take(max)
                .collect::<String>()
                .trim()
                .to_string()
        };
        let machine = clean(&r.machine, MAX_REPORT_MACHINE_LEN);
        Ok(DeviceReport {
            machine: if machine.is_empty() {
                "?".into()
            } else {
                machine
            },
            level: r.level.clone(),
            source: r.source.clone(),
            message: clean(&r.message, MAX_REPORT_MESSAGE_LEN),
            timestamp: r.timestamp,
        })
    }

    pub fn validate_json(json_str: &str) -> anyhow::Result<()> {
        serde_json::from_str::<serde_json::Value>(json_str)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> StateSnapshot {
        StateSnapshot {
            project_path: "C:\\Users\\dev\\project".into(),
            file_name: "main.rs".into(),
            cursor_line: 42,
            clipboard_content: "hello".into(),
            timestamp: 1,
            source_node_id: "node-1".into(),
            browser_url: String::new(),
            file_content: String::new(),
            file_rel_path: String::new(),
            ..Default::default()
        }
    }

    #[test]
    fn path_traversal_rejected() {
        assert!(Validator::validate_path("..\\..\\etc\\passwd").is_err());
        assert!(Validator::validate_path("C:\\Users\\dev\\..\\admin").is_err());
        assert!(Validator::validate_path("C:\\Users\\dev\\project").is_ok());
    }

    #[test]
    fn control_chars_rejected() {
        assert!(Validator::validate_path("C:\\evil\0path").is_err());
        assert!(Validator::validate_file_name("bad\nname.rs").is_err());
    }

    #[test]
    fn file_name_separators_rejected() {
        assert!(Validator::validate_file_name("..\\evil.rs").is_err());
        assert!(Validator::validate_file_name("sub/dir.rs").is_err());
        assert!(Validator::validate_file_name("main.rs").is_ok());
    }

    #[test]
    fn ip_validation() {
        assert!(Validator::validate_ip_address("192.168.1.1").is_ok());
        assert!(Validator::validate_ip_address("::1").is_ok());
        assert!(Validator::validate_ip_address("not.an.ip").is_err());
    }

    #[test]
    fn port_validation() {
        assert!(Validator::validate_port(9001).is_ok());
        assert!(Validator::validate_port(80).is_err());
    }

    #[test]
    fn node_id_validation() {
        assert!(Validator::validate_node_id("abc-123_def").is_ok());
        assert!(Validator::validate_node_id("bad;rm -rf").is_err());
        assert!(Validator::validate_node_id("").is_err());
    }

    #[test]
    fn valid_snapshot_passes() {
        assert!(Validator::validate_snapshot(&snapshot()).is_ok());
    }

    #[test]
    fn oversized_clipboard_rejected() {
        let mut s = snapshot();
        s.clipboard_content = "x".repeat(MAX_CLIPBOARD_SIZE + 1);
        assert!(Validator::validate_snapshot(&s).is_err());
    }

    #[test]
    fn malicious_project_path_rejected() {
        let mut s = snapshot();
        s.project_path = "C:\\good\\..\\..\\Windows\\System32".into();
        assert!(Validator::validate_snapshot(&s).is_err());
    }

    /// A browser-tab capture has no VS Code file/path — it must still be valid,
    /// otherwise the receiving machine rejects the DROP ("path is empty").
    #[test]
    fn browser_only_snapshot_is_valid() {
        let mut s = snapshot();
        s.project_path = String::new();
        s.file_name = String::new();
        s.clipboard_content = String::new();
        s.browser_url = "https://github.com/palmsync".into();
        assert!(Validator::validate_snapshot(&s).is_ok());
    }

    #[test]
    fn clipboard_only_snapshot_is_valid() {
        let mut s = snapshot();
        s.project_path = String::new();
        s.file_name = String::new();
        s.browser_url = String::new();
        s.clipboard_content = "just some copied text".into();
        assert!(Validator::validate_snapshot(&s).is_ok());
    }

    #[test]
    fn completely_empty_snapshot_rejected() {
        let mut s = snapshot();
        s.project_path = String::new();
        s.file_name = String::new();
        s.browser_url = String::new();
        s.clipboard_content = String::new();
        assert!(Validator::validate_snapshot(&s).is_err());
    }

    #[test]
    fn rel_path_traversal_rejected() {
        assert!(Validator::validate_rel_path("src/main.rs").is_ok());
        assert!(Validator::validate_rel_path("src\\lib\\mod.rs").is_ok());
        assert!(Validator::validate_rel_path("../evil.rs").is_err());
        assert!(Validator::validate_rel_path("src/../../evil.rs").is_err());
        assert!(Validator::validate_rel_path("C:\\Windows\\evil.dll").is_err());
        assert!(Validator::validate_rel_path("/etc/passwd").is_err());
        assert!(Validator::validate_rel_path("\\\\server\\share").is_err());

        let mut s = snapshot();
        s.file_rel_path = "..\\..\\Startup\\x.bat".into();
        assert!(Validator::validate_snapshot(&s).is_err());
    }

    #[test]
    fn file_content_size_limited() {
        let mut s = snapshot();
        s.file_content = "fn main() {}".into();
        assert!(Validator::validate_snapshot(&s).is_ok());
        s.file_content = "x".repeat(MAX_FILE_CONTENT_SIZE + 1);
        assert!(Validator::validate_snapshot(&s).is_err());
    }

    #[test]
    fn report_is_sanitized_for_the_host_log() {
        let r = DeviceReport {
            machine: "PC-2\n2026-01-01 [ERROR] fake: injected".into(),
            level: "warn".into(),
            source: "kamera".into(),
            message: format!("Kamera açılamadı\r\nikinci satır{}", "x".repeat(5000)),
            timestamp: 1,
        };
        let s = Validator::sanitize_report(&r).unwrap();
        assert!(
            !s.machine.contains('\n') && !s.message.contains('\n') && !s.message.contains('\r')
        );
        assert!(s.message.chars().count() <= MAX_REPORT_MESSAGE_LEN);
        assert!(s.message.starts_with("Kamera açılamadı"));

        let mut bad = r.clone();
        bad.level = "critical".into();
        assert!(Validator::sanitize_report(&bad).is_err());
        let mut bad = r.clone();
        bad.source = "../x".into();
        assert!(Validator::sanitize_report(&bad).is_err());
        let mut bad = r;
        bad.message = "   ".into();
        assert!(Validator::sanitize_report(&bad).is_err());
    }

    #[test]
    fn app_only_snapshot_with_screenshot_is_valid() {
        let mut s = snapshot();
        s.project_path = String::new();
        s.file_name = String::new();
        s.clipboard_content = String::new();
        s.app.process = "figma".into();
        s.app.kind = "other".into();
        s.app.display_name = "Figma".into();
        s.screenshot_jpeg_b64 = "/9j/4AAQ".into();
        assert!(Validator::validate_snapshot(&s).is_ok());
    }

    #[test]
    fn app_fields_are_validated() {
        let mut s = snapshot();
        s.app.process = "evil\\..\\cmd".into();
        assert!(Validator::validate_snapshot(&s).is_err());

        let mut s = snapshot();
        s.app.kind = "installer".into();
        assert!(Validator::validate_snapshot(&s).is_err());

        let mut s = snapshot();
        s.app.window_title = "line1\nline2".into();
        assert!(Validator::validate_snapshot(&s).is_err());

        let mut s = snapshot();
        s.app.document_path = "%USERPROFILE%\\..\\..\\Windows\\x".into();
        assert!(Validator::validate_snapshot(&s).is_err());

        let mut s = snapshot();
        s.app.process = "notepad++".into();
        s.app.exe_path = "%ProgramFiles%\\Notepad++\\notepad++.exe".into();
        assert!(Validator::validate_snapshot(&s).is_ok());
    }

    #[test]
    fn binary_file_data_is_validated() {
        let mut s = snapshot();
        s.file_data_b64 = "UEsDBA==".into();
        assert!(Validator::validate_snapshot(&s).is_ok());

        s.file_data_b64 = "not base64!".into();
        assert!(Validator::validate_snapshot(&s).is_err());

        s.file_data_b64 = "UEsDBA==".into();
        s.file_content = "text too".into();
        assert!(Validator::validate_snapshot(&s).is_err(), "text and binary both set");

        let mut s = snapshot();
        s.file_name = String::new();
        s.file_data_b64 = "UEsDBA==".into();
        assert!(Validator::validate_snapshot(&s).is_err(), "data without a name");

        let mut s = snapshot();
        s.file_data_b64 = "A".repeat(MAX_FILE_DATA_B64_LEN + 4);
        assert!(Validator::validate_snapshot(&s).is_err());
    }

    #[test]
    fn oversized_screenshot_rejected() {
        let mut s = snapshot();
        s.screenshot_jpeg_b64 = "A".repeat(MAX_SCREENSHOT_B64_LEN + 4);
        assert!(Validator::validate_snapshot(&s).is_err());
    }

    #[test]
    fn non_http_browser_url_rejected() {
        let mut s = snapshot();
        s.browser_url = "file:///C:/Windows/System32/config".into();
        assert!(Validator::validate_snapshot(&s).is_err());
    }
}
