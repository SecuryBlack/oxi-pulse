use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::time;
use tracing::info;

mod config;
mod management;
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

    let status_handle =
        sb_agent_core::status::StatusHandle::new("oxipulse", env!("CARGO_PKG_VERSION"));
    sb_agent_core::status::spawn_server(
        status_handle.clone(),
        sb_agent_core::status::default_socket_path("oxipulse"),
    );

    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_all();
    let cpu_model = sys.cpus().first().map(|c| c.brand().trim().to_string());
    let (os_name, os_version) = match std::env::var("OXIPULSE_HOST_ROOT")
        .ok()
        .and_then(|root| metrics::host_os_release(&root))
    {
        Some((name, version)) => (Some(name), version),
        None => (sysinfo::System::name(), sysinfo::System::os_version()),
    };
    let kernel_version = sysinfo::System::kernel_version();

    let host_info = telemetry::HostInfo {
        host_name: sysinfo::System::host_name(),
        cpu_model,
        os_name,
        os_version,
        kernel_version,
    };

    let metrics_counter: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let buffer_len_atomic: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let is_offline_atomic: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // The token is sent as a Bearer header unless `headers` sets its own authorization
    let mut headers = cfg.headers.clone();
    if !cfg.token.is_empty() {
        headers
            .entry("authorization".to_string())
            .or_insert_with(|| format!("Bearer {}", cfg.token));
    }

    let (instruments, _provider) = match telemetry::init(
        &cfg.endpoint,
        &headers,
        cfg.interval_secs,
        cfg.buffer_max_size,
        Arc::clone(&buffer_len_atomic),
        Arc::clone(&is_offline_atomic),
        host_info,
    ) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to initialise OTLP exporter: {}", e);
            std::process::exit(1);
        }
    };

    info!("OTLP exporter initialised");
    status_handle.set_state("running");

    if cfg.auto_update {
        sb_agent_core::updater::start_daily_check(sb_agent_core::updater::UpdaterConfig::new(
            "securyblack",
            "oxi-pulse",
            "oxipulse",
            env!("CARGO_PKG_VERSION"),
        ));
    } else {
        info!("auto-update disabled by config");
    }

    let command_registry = sb_agent_core::command_intake::CommandRegistry::new();
    management::register(&command_registry);
    sb_agent_core::command_intake::spawn_server(
        command_registry,
        sb_agent_core::command_intake::default_socket_path("oxipulse"),
        sb_agent_core::command_intake::SigningContext {
            server_token: cfg.token.clone(),
            target_agent: "oxipulse".into(),
        },
    );

    // ── Telemetry opt-in ─────────────────────────────────────────────────────
    // Resolve effective telemetry flag:
    //   Some(true)  → explicit opt-in (local config / env var)
    //   Some(false) → explicit opt-out — never fetch remote config
    //   None        → off, except for SecuryBlack Cloud endpoints, where the
    //                 account's server-side setting decides. Never send the
    //                 token of a third-party OTLP backend to SecuryBlack.
    let telemetry_active = match cfg.telemetry_enabled {
        Some(v) => v,
        None if !cfg.is_securyblack_endpoint() || cfg.token.is_empty() => false,
        None => {
            info!(api_url = %cfg.api_url, "fetching remote config");
            match phone_home::fetch_remote_config(&cfg.api_url, &cfg.token).await {
                Some(rc) => {
                    info!(
                        telemetry_enabled = rc.telemetry_enabled,
                        "remote config received"
                    );
                    rc.telemetry_enabled
                }
                None => {
                    info!("remote config unavailable, telemetry disabled");
                    false
                }
            }
        }
    };

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

    // Snapshots are always recorded. The exporter keeps what the backend does
    // not accept (up to buffer_max_size) and sends it once it is reachable.
    let mut collector = metrics::Collector::new();
    let mut interval = time::interval(Duration::from_secs(cfg.interval_secs));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let m = collector.collect(&cfg.latency_targets).await;
                telemetry::record(&instruments, &m);
                metrics_counter.fetch_add(1, Ordering::Relaxed);
                info!(
                    cpu = format!("{:.1}%", m.cpu_usage_percent),
                    ram_used_mb = m.ram_used_bytes / 1024 / 1024,
                    ram_total_mb = m.ram_total_bytes / 1024 / 1024,
                    disks = m.disks.iter().map(|d| format!("{}={:.1}%", d.name, d.used_bytes as f64 / d.total_bytes as f64 * 100.0)).collect::<Vec<_>>().join(", "),
                    net_in_kb = m.networks.iter().map(|n| n.received).sum::<u64>() / 1024,
                    net_out_kb = m.networks.iter().map(|n| n.transmitted).sum::<u64>() / 1024,
                    latencies = m.latencies.iter().map(|l| {
                        let val = l.latency_ms.map(|ms| format!("{:.1}ms", ms)).unwrap_or_else(|| "fail".to_string());
                        format!("{}={}", l.target, val)
                    }).collect::<Vec<_>>().join(", "),
                    "metrics collected and recorded"
                );
                status_handle.set_details(serde_json::json!({
                    "cpu_usage_percent": m.cpu_usage_percent,
                    "cpu_count": m.cpu_count,
                    "ram_used_bytes": m.ram_used_bytes,
                    "ram_total_bytes": m.ram_total_bytes,
                    "uptime_secs": m.uptime_secs,
                    "buffered": buffer_len_atomic.load(Ordering::Relaxed),
                    "offline": is_offline_atomic.load(Ordering::Relaxed),
                }));
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

fn check_cli_args() {
    sb_agent_core::cli::dispatch_common_args("oxipulse", "oxipulse", env!("CARGO_PKG_VERSION"));
}

#[cfg(windows)]
fn main() {
    check_cli_args();
    // ERROR_FAILED_SERVICE_CONTROLLER_CONNECT (1063): process was not started
    // by the SCM, so run in console mode instead.
    match sb_agent_core::service::windows::run_service("OxiPulse", run) {
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
