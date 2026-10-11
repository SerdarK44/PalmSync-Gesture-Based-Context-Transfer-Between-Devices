use clipboard_win::{formats, get_clipboard, set_clipboard};

use palmsync_shared::audit;
use palmsync_shared::validation::MAX_CLIPBOARD_SIZE;

/// Patterns that indicate highly sensitive clipboard content.
/// Detection only warns + audits; content is still synced because
/// the transport is end-to-end encrypted.
const SENSITIVE_PATTERNS: &[&str] = &[
    "BEGIN PRIVATE KEY",
    "BEGIN RSA PRIVATE KEY",
    "BEGIN OPENSSH PRIVATE KEY",
    "BEGIN PGP PRIVATE KEY",
    "aws_secret_access_key",
];

pub fn get_text() -> anyhow::Result<String> {
    let text: String =
        get_clipboard(formats::Unicode).map_err(|e| anyhow::anyhow!("clipboard read: {}", e))?;

    if text.len() > MAX_CLIPBOARD_SIZE {
        audit::log_security("clipboard_too_large", &format!("size={}", text.len()));
        anyhow::bail!(
            "clipboard content too large ({} bytes > {} max)",
            text.len(),
            MAX_CLIPBOARD_SIZE
        );
    }

    if let Some(pattern) = detect_sensitive(&text) {
        audit::log_security("sensitive_clipboard", pattern);
        log::warn!(
            "Sensitive content ({}) detected in clipboard — it will be synced E2E-encrypted",
            pattern
        );
    }

    Ok(text)
}

pub fn set_text(content: &str) -> anyhow::Result<()> {
    if content.len() > MAX_CLIPBOARD_SIZE {
        anyhow::bail!("refusing to write oversized clipboard content");
    }
    set_clipboard(formats::Unicode, content)
        .map_err(|e| anyhow::anyhow!("clipboard write: {}", e))?;
    Ok(())
}

/// Overwrite the clipboard before clearing so the previous content
/// is not trivially recoverable from clipboard history tools.
/// Exposed for a future "panic wipe" command (Faz 7 CLI).
#[allow(dead_code)]
pub fn clear_secure() -> anyhow::Result<()> {
    let filler = "0".repeat(4096);
    for _ in 0..3 {
        set_clipboard(formats::Unicode, filler.as_str())
            .map_err(|e| anyhow::anyhow!("clipboard wipe: {}", e))?;
    }
    set_clipboard(formats::Unicode, "").map_err(|e| anyhow::anyhow!("clipboard clear: {}", e))?;
    audit::log_security("clipboard_cleared", "secure wipe");
    Ok(())
}

fn detect_sensitive(content: &str) -> Option<&'static str> {
    SENSITIVE_PATTERNS
        .iter()
        .find(|p| content.contains(**p))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_private_keys() {
        assert!(detect_sensitive("-----BEGIN PRIVATE KEY-----\nMII...").is_some());
        assert!(detect_sensitive("normal rust code fn main() {}").is_none());
    }

    #[test]
    fn oversized_set_rejected() {
        let big = "x".repeat(MAX_CLIPBOARD_SIZE + 1);
        assert!(set_text(&big).is_err());
    }
}
