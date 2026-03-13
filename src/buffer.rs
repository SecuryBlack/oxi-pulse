use std::collections::VecDeque;
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::metrics::Metrics;

/// Ring buffer that stores metric snapshots while the collector is unreachable.
/// When full, the oldest entry is dropped to make room for the newest.
pub struct OfflineBuffer {
    queue: VecDeque<Metrics>,
    max_size: usize,
}

impl OfflineBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(max_size.min(1024)),
            max_size,
        }
    }

    pub fn push(&mut self, m: Metrics) {
        if self.queue.len() >= self.max_size {
            self.queue.pop_front();
            warn!(max = self.max_size, "buffer full — dropping oldest snapshot");
        }
        self.queue.push_back(m);
    }

    pub fn drain_all(&mut self) -> Vec<Metrics> {
        self.queue.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

/// Exponential backoff for connectivity checks.
/// Doubles the wait (in ticks) after each failure, up to `max_ticks`.
pub struct Backoff {
    current_ticks: u64,
    max_ticks: u64,
    countdown: u64,
}

impl Backoff {
    /// `max_wait_secs` / `interval_secs` sets the ceiling in ticks (min 1).
    pub fn new(interval_secs: u64) -> Self {
        let max_ticks = (300 / interval_secs).max(1); // ceil at ~5 minutes
        Self {
            current_ticks: 1,
            max_ticks,
            countdown: 0,
        }
    }

    /// Returns true if it is time to attempt a connectivity check this tick.
    pub fn should_check(&mut self) -> bool {
        if self.countdown == 0 {
            true
        } else {
            self.countdown -= 1;
            false
        }
    }

    /// Call after a failed check to back off.
    pub fn on_failure(&mut self) {
        self.countdown = self.current_ticks;
        self.current_ticks = (self.current_ticks * 2).min(self.max_ticks);
    }

    /// Call after a successful check to reset.
    pub fn on_success(&mut self) {
        self.current_ticks = 1;
        self.countdown = 0;
    }
}

/// Attempt a TCP connection to the host:port extracted from an OTLP endpoint URL.
/// Returns true if reachable, false otherwise.
pub async fn is_reachable(endpoint: &str) -> bool {
    let addr = match parse_host_port(endpoint) {
        Some(a) => a,
        None => {
            warn!(%endpoint, "could not parse endpoint host:port");
            return false;
        }
    };

    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

/// Extract "host:port" from URLs like "http://host:4317" or "https://host:4317".
fn parse_host_port(endpoint: &str) -> Option<String> {
    let without_scheme = endpoint
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    // Already host:port or host
    let addr = if without_scheme.contains(':') {
        without_scheme.to_string()
    } else {
        // Default gRPC port
        format!("{}:4317", without_scheme)
    };

    Some(addr)
}

/// Log a status change clearly.
pub fn log_status_change(was_offline: bool, now_offline: bool, buffered: usize) {
    match (was_offline, now_offline) {
        (false, true) => warn!("collector unreachable — switching to offline mode"),
        (true, false) => info!(flushing = buffered, "collector reachable — reconnected"),
        _ => {}
    }
}
