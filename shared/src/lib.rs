pub mod audit;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod messages;
pub mod validation;

pub use config::{config_dir, config_file_path, Config};
pub use messages::*;

/// First up to `n` characters of `s`, never panicking on short strings or
/// multi-byte UTF-8 boundaries. Used for abbreviating node ids in logs —
/// node ids come from machine names, which can be shorter than 8 chars.
pub fn short(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
