use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::time;
use tracing::info;

mod config;
mod metrics;
mod phone_home;
mod telemetry;

/// Core agent loop. Runs until the shutdown receiver fires.
async fn run(mut shutdown: tokio::sync::oneshot::Receiver<()>) {
    let log_dir = sb_agent_core::config::default_config_path("oxipulse")
        .parent()
        .expect("config path always has a parent")
        .to_path_buf();
    sb_agent_core::logging::init("oxipulse", &log_dir, "info");

    info!("OxiPulse v{} starting", env!("CARGO_PKG_VERSION"));

    let cfg = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{}", e);
            std::process::exit(1);
        }
    };

    info!(endpoint = %cfg.endpoint, interval_secs = cfg.interval_secs, "config loaded");

    let status_handle = sb_agent_core::status::StatusHandle::new("oxipulse", env!("CARGO_PKG_VERSION"));
    sb_agent_core::status::spawn_server(
        status_handle.clone(),
        sb_agent_core::status::default_socket_path("oxipulse"),
    );

    let (instruments, _provider) = match telemetry::init(&cfg.endpoint, &cfg.token, cfg.interval_secs) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to initialise OTLP exporter: {}", e);
            std::process::exit(1);
        }
    };

    info!("OTLP exporter initialised");
    status_handle.set_state("running");

    sb_agent_core::updater::start_daily_check(sb_agent_core::updater::UpdaterConfig::new(
        "securyblack",
        "oxi-pulse",
        "oxipulse",
        env!("CARGO_PKG_VERSION"),
    ));

    // ── Telemetry opt-in ─────────────────────────────────────────────────────
    // Resolve effective telemetry flag:
    //   Some(true)  → explicit opt-in (local config / env var)
    //   Some(false) → explicit opt-out — never fetch remote config
    //   None        → defer to server-side config fetched from the API
    let telemetry_active = match cfg.telemetry_enabled {
        Some(v) => v,
        None => {
            info!(api_url = %cfg.api_url, "fetching remote config");
            match phone_home::fetch_remote_config(&cfg.api_url, &cfg.token).await {
                Some(rc) => {
                    info!(telemetry_enabled = rc.telemetry_enabled, "remote config received");
                    rc.telemetry_enabled
                }
                None => {
                    info!("remote config unavailable, telemetry disabled");
                    false
                }
            }
        }
    };

    let metrics_counter: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let buffer_len_atomic: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let is_offline_atomic: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    if telemetry_active {
        info!("telemetry enabled — usage pings will be sent every 24 h");
        phone_home::start_telemetry_task(
            cfg.api_url.clone(),
            cfg.token.clone(),
            cfg.interval_secs,
            cfg.buffer_max_size,
            Arc::clone(&metrics_counter),
            Arc::clone(&buffer_len_atomic),
            Arc::clone(&is_offline_atomic),
        );
    }
    // ─────────────────────────────────────────────────────────────────────────

    let mut collector = metrics::Collector::new();
    let mut offline_buffer = sb_agent_core::buffer::OfflineBuffer::new(cfg.buffer_max_size);
    let mut backoff = sb_agent_core::buffer::Backoff::new(cfg.interval_secs);
    let mut is_offline = false;
    let mut interval = time::interval(Duration::from_secs(cfg.interval_secs));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let m = collector.collect(&cfg.latency_targets, &cfg.endpoint).await;

                let did_check = !is_offline || backoff.should_check();
                let reachable = if did_check {
                    sb_agent_core::net::is_reachable(&cfg.endpoint).await
                } else {
                    false
                };

                if reachable {
                    sb_agent_core::buffer::log_status_change(is_offline, false, offline_buffer.len());

                    if is_offline {
                        let buffered = offline_buffer.drain_all();
                        let count = buffered.len();
                        for bm in buffered {
                            telemetry::record(&instruments, &bm);
                            metrics_counter.fetch_add(1, Ordering::Relaxed);
                        }
                        info!(flushed = count, "buffer flushed");
                        backoff.on_success();
                        is_offline = false;
                        is_offline_atomic.store(false, Ordering::Relaxed);
                    }

                    telemetry::record(&instruments, &m);
                    metrics_counter.fetch_add(1, Ordering::Relaxed);
                    buffer_len_atomic.store(offline_buffer.len() as u64, Ordering::Relaxed);
                    info!(
                        cpu = format!("{:.1}%", m.cpu_usage_percent),
                        ram_used_mb = m.ram_used_bytes / 1024 / 1024,
                        ram_total_mb = m.ram_total_bytes / 1024 / 1024,
                        disks = m.disks.iter().map(|d| format!("{}={:.1}%", d.name, d.used_bytes as f64 / d.total_bytes as f64 * 100.0)).collect::<Vec<_>>().join(", "),
                        net_in_kbps = m.net_bps_in / 1024.0,
                        net_out_kbps = m.net_bps_out / 1024.0,
                        latencies = m.latencies.iter().map(|l| {
                            let val = l.latency_ms.map(|ms| format!("{:.1}ms", ms)).unwrap_or_else(|| "fail".to_string());
                            format!("{}={}", l.target, val)
                        }).collect::<Vec<_>>().join(", "),
                        "metrics collected and recorded"
                    );
                    status_handle.set_details(serde_json::json!({
                        "cpu_usage_percent": m.cpu_usage_percent,
                        "ram_used_bytes": m.ram_used_bytes,
                        "ram_total_bytes": m.ram_total_bytes,
                        "buffered": offline_buffer.len(),
                        "offline": false,
                    }));
                } else {
                    sb_agent_core::buffer::log_status_change(is_offline, true, 0);
                    is_offline = true;
                    is_offline_atomic.store(true, Ordering::Relaxed);
                    if did_check {
                        backoff.on_failure();
                    }

                    offline_buffer.push(m);
                    buffer_len_atomic.store(offline_buffer.len() as u64, Ordering::Relaxed);
                    tracing::warn!(buffered = offline_buffer.len(), max = cfg.buffer_max_size, "offline — buffering metrics");
                    status_handle.set_details(serde_json::json!({
                        "buffered": offline_buffer.len(),
                        "offline": true,
                    }));
                }
            }
            _ = &mut shutdown => {
                info!("shutdown signal received, flushing telemetry provider");
                status_handle.set_state("stopping");
                if let Err(e) = _provider.shutdown() {
                    tracing::warn!("failed to shutdown OTLP exporter cleanly: {}", e);
                }
                info!("stopped");
                break;
            }
        }
    }
}


/// Maneja `--version`/`-V`, `status` y `top` antes de arrancar como
/// servicio/consola. Sale del proceso si alguno aplica.
fn check_cli_args() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        return;
    }
    match args[1].as_str() {
        "--version" | "-V" => {
            println!("oxipulse {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        "status" => {
            match sb_agent_core::status_client::read_once("oxipulse") {
                Ok(payload) => {
                    println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("[oxipulse] {e}");
                    std::process::exit(1);
                }
            }
        }
        "top" => {
            if let Err(e) = sb_agent_core::tui::run_top("oxipulse") {
                eprintln!("[oxipulse] {e}");
                std::process::exit(1);
            }
            std::process::exit(0);
        }
        _ => {}
    }
}

#[cfg(windows)]
fn main() {
    check_cli_args();
    // ERROR_FAILED_SERVICE_CONTROLLER_CONNECT (1063): process was not started
    // by the SCM, so run in console mode instead.
    match sb_agent_core::service::windows::run_service("OxiPulse", |rx| run(rx)) {
        Ok(_) => {}
        Err(e) if sb_agent_core::service::windows::is_not_started_by_scm(&e) => {
            sb_agent_core::service::run_console(run);
        }
        Err(e) => {
            eprintln!("[oxipulse] service error: {e}");
            std::process::exit(1);
        }
    }
}

// ── Linux / macOS ─────────────────────────────────────────────────────────────

#[cfg(not(windows))]
fn main() {
    check_cli_args();
    sb_agent_core::service::run_console(run);
}
