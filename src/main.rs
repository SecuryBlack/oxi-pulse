use std::time::Duration;
use tokio::time;
use tracing::info;

mod buffer;
mod config;
mod metrics;
mod telemetry;
mod updater;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("OxiPulse v{} starting", env!("CARGO_PKG_VERSION"));

    let cfg = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{}", e);
            std::process::exit(1);
        }
    };

    info!(endpoint = %cfg.endpoint, interval_secs = cfg.interval_secs, "config loaded");

    let (instruments, _provider) = match telemetry::init(&cfg.endpoint, &cfg.token, cfg.interval_secs) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to initialise OTLP exporter: {}", e);
            std::process::exit(1);
        }
    };

    info!("OTLP exporter initialised");

    // Start background daily update checker
    updater::start_daily_check();

    let mut collector = metrics::Collector::new();
    let mut offline_buffer = buffer::OfflineBuffer::new(cfg.buffer_max_size);
    let mut backoff = buffer::Backoff::new(cfg.interval_secs);
    let mut is_offline = false;
    let mut interval = time::interval(Duration::from_secs(cfg.interval_secs));

    loop {
        interval.tick().await;

        // Always collect — never drop a reading regardless of connectivity
        let m = collector.collect();

        // Check connectivity (with backoff when offline to avoid hammering the endpoint)
        let reachable = if !is_offline || backoff.should_check() {
            buffer::is_reachable(&cfg.endpoint).await
        } else {
            false
        };

        if reachable {
            // Log transition offline → online
            buffer::log_status_change(is_offline, false, offline_buffer.len());

            // Flush buffered snapshots first, then record the current one
            if is_offline {
                let buffered = offline_buffer.drain_all();
                let count = buffered.len();
                for bm in buffered {
                    telemetry::record(&instruments, &bm);
                }
                info!(flushed = count, "buffer flushed");
                backoff.on_success();
                is_offline = false;
            }

            telemetry::record(&instruments, &m);
            info!(
                cpu = format!("{:.1}%", m.cpu_usage_percent),
                ram_used_mb = m.ram_used_bytes / 1024 / 1024,
                ram_total_mb = m.ram_total_bytes / 1024 / 1024,
                disk_used_gb = m.disk_used_bytes / 1024 / 1024 / 1024,
                disk_total_gb = m.disk_total_bytes / 1024 / 1024 / 1024,
                net_in_kb = m.net_bytes_in / 1024,
                net_out_kb = m.net_bytes_out / 1024,
                "metrics collected and recorded"
            );
        } else {
            // Log transition online → offline
            buffer::log_status_change(is_offline, true, 0);
            is_offline = true;
            backoff.on_failure();

            offline_buffer.push(m);
            tracing::warn!(buffered = offline_buffer.len(), max = cfg.buffer_max_size, "offline — buffering metrics");
        }
    }
}
