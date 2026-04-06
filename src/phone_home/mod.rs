use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
pub struct RemoteConfig {
    pub telemetry_enabled: bool,
}

#[derive(Debug, Serialize)]
struct TelemetryPing<'a> {
    agent_version: &'a str,
    os: &'a str,
    arch: &'a str,
    interval_secs: u64,
    uptime_secs: u64,
    metrics_exported_total: u64,
    buffer_occupancy_pct: f64,
    last_error_kind: &'a str,
}

/// Fetches the server-side remote config for this agent.
/// Returns `None` if the request fails or the agent token is invalid.
pub async fn fetch_remote_config(api_url: &str, token: &str) -> Option<RemoteConfig> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;

    let url = format!("{}/agents/me/config", api_url);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        warn!(status = %resp.status(), "remote config fetch returned non-2xx");
        return None;
    }

    resp.json::<RemoteConfig>().await.ok()
}

/// Spawns a background task that sends a telemetry ping once on startup (after a
/// short delay) and then every 24 hours.
pub fn start_telemetry_task(
    api_url: String,
    token: String,
    interval_secs: u64,
    buffer_max: usize,
    metrics_counter: Arc<AtomicU64>,
    buffer_len: Arc<AtomicU64>,
    is_offline: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        let started_at = Instant::now();

        // Wait 60 s before the first ping so the agent has time to settle.
        tokio::time::sleep(Duration::from_secs(60)).await;

        loop {
            let uptime_secs = started_at.elapsed().as_secs();
            let exported = metrics_counter.load(Ordering::Relaxed);
            let buf_len = buffer_len.load(Ordering::Relaxed) as f64;
            let offline = is_offline.load(Ordering::Relaxed);
            let occupancy_pct = if buffer_max > 0 {
                buf_len / buffer_max as f64 * 100.0
            } else {
                0.0
            };

            let ping = TelemetryPing {
                agent_version: env!("CARGO_PKG_VERSION"),
                os: std::env::consts::OS,
                arch: std::env::consts::ARCH,
                interval_secs,
                uptime_secs,
                metrics_exported_total: exported,
                buffer_occupancy_pct: occupancy_pct,
                last_error_kind: if offline { "connectivity" } else { "none" },
            };

            send_ping(&api_url, &token, &ping).await;

            tokio::time::sleep(Duration::from_secs(24 * 3600)).await;
        }
    });
}

async fn send_ping(api_url: &str, token: &str, ping: &TelemetryPing<'_>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let url = format!("{}/agents/me/telemetry", api_url);
    match client.post(&url).bearer_auth(token).json(ping).send().await {
        Ok(resp) if resp.status().is_success() => {
            info!("telemetry ping sent");
        }
        Ok(resp) => {
            warn!(status = %resp.status(), "telemetry ping rejected by server");
        }
        Err(e) => {
            warn!(error = %e, "telemetry ping failed");
        }
    }
}
