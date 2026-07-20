use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// Maximum accepted WebSocket text payload (bytes).
/// Snapshots carry up to 10MB clipboard + JSON/hex overhead.
pub const MAX_PAYLOAD_SIZE: usize = 32 * 1024 * 1024;

/// Sliding-window per-IP rate limiter.
#[derive(Clone)]
pub struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    max_requests: u32,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            windows: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Default for connection attempts: 10 per 60 seconds per IP.
    pub fn for_connections() -> Self {
        Self::new(10, 60)
    }

    /// Default for messages: 100 per 10 seconds per client.
    pub fn for_messages() -> Self {
        Self::new(100, 10)
    }

    pub fn check(&self, client_key: &str) -> anyhow::Result<()> {
        let mut windows = self.windows.lock();
        let now = Instant::now();

        // Opportunistic cleanup of expired windows
        if windows.len() > 1024 {
            let window = self.window;
            windows.retain(|_, (_, start)| now.duration_since(*start) <= window);
        }

        match windows.get_mut(client_key) {
            Some((count, start)) => {
                if now.duration_since(*start) > self.window {
                    *count = 1;
                    *start = now;
                } else if *count >= self.max_requests {
                    anyhow::bail!("rate limit exceeded for {}", client_key);
                } else {
                    *count += 1;
                }
            }
            None => {
                windows.insert(client_key.to_string(), (1, now));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_limit() {
        let limiter = RateLimiter::new(5, 60);
        for _ in 0..5 {
            assert!(limiter.check("192.168.1.1").is_ok());
        }
        assert!(limiter.check("192.168.1.1").is_err());
    }

    #[test]
    fn separate_clients_tracked_independently() {
        let limiter = RateLimiter::new(2, 60);
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_err());
        assert!(limiter.check("b").is_ok());
    }

    #[test]
    fn window_resets() {
        let limiter = RateLimiter::new(1, 0); // 0-second window: resets immediately
        assert!(limiter.check("x").is_ok());
        std::thread::sleep(Duration::from_millis(10));
        assert!(limiter.check("x").is_ok());
    }
}
