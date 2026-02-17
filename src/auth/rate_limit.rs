use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const MAX_ATTEMPTS: usize = 5;
const WINDOW_SECS: u64 = 900; // 15 minutes

#[derive(Clone)]
pub struct RateLimiter {
    attempts: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if the given IP is rate-limited. Returns true if blocked.
    /// Also lazily cleans up stale entries for the checked IP.
    pub fn is_blocked(&self, ip: IpAddr) -> bool {
        let mut map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        let cutoff = Instant::now() - std::time::Duration::from_secs(WINDOW_SECS);

        if let Some(timestamps) = map.get_mut(&ip) {
            timestamps.retain(|t| *t > cutoff);
            timestamps.len() >= MAX_ATTEMPTS
        } else {
            false
        }
    }

    /// Record a failed login attempt for the given IP.
    pub fn record_failure(&self, ip: IpAddr) {
        let mut map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(ip).or_default().push(Instant::now());
    }

    /// Clear all recorded attempts for the given IP (call on successful login).
    pub fn clear(&self, ip: IpAddr) {
        let mut map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&ip);
    }
}
