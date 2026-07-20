use crate::StateSnapshot;

pub const MAX_PATH_LEN: usize = 4096;
pub const MAX_FILE_NAME_LEN: usize = 512;
pub const MAX_CLIPBOARD_SIZE: usize = 10 * 1024 * 1024; // 10 MB
pub const MAX_NODE_ID_LEN: usize = 128;

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
    pub fn validate_snapshot(snap: &StateSnapshot) -> anyhow::Result<()> {
        Self::validate_path(&snap.project_path)?;
        Self::validate_file_name(&snap.file_name)?;
        Self::validate_node_id(&snap.source_node_id)?;
        if snap.clipboard_content.len() > MAX_CLIPBOARD_SIZE {
            anyhow::bail!(
                "clipboard content too large ({} bytes)",
                snap.clipboard_content.len()
            );
        }
        if snap.cursor_line > 10_000_000 {
            anyhow::bail!("cursor line out of range");
        }
        Ok(())
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
}
