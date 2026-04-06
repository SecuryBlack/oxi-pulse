use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::time;
use tracing::info;

mod buffer;
mod config;
mod metrics;
mod phone_home;
mod telemetry;
mod updater;

#[cfg(windows)]
fn init_logging() {
    let log_dir = r"C:\ProgramData\oxipulse";
    let file_appender = tracing_appender::rolling::daily(log_dir, "oxipulse.log");
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(file_appender)
        .with_ansi(false)
        .init();
}

#[cfg(not(windows))]
fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

/// Core agent loop. Runs until the shutdown receiver fires.
async fn run(mut shutdown: tokio::sync::oneshot::Receiver<()>) {
    init_logging();

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

    updater::start_daily_check();

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
    let mut offline_buffer = buffer::OfflineBuffer::new(cfg.buffer_max_size);
    let mut backoff = buffer::Backoff::new(cfg.interval_secs);
    let mut is_offline = false;
    let mut interval = time::interval(Duration::from_secs(cfg.interval_secs));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let m = collector.collect();

                let did_check = !is_offline || backoff.should_check();
                let reachable = if did_check {
                    buffer::is_reachable(&cfg.endpoint).await
                } else {
                    false
                };

                if reachable {
                    buffer::log_status_change(is_offline, false, offline_buffer.len());

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
                        disk_used_gb = m.disk_used_bytes / 1024 / 1024 / 1024,
                        disk_total_gb = m.disk_total_bytes / 1024 / 1024 / 1024,
                        net_in_kb = m.net_bytes_in / 1024,
                        net_out_kb = m.net_bytes_out / 1024,
                        "metrics collected and recorded"
                    );
                } else {
                    buffer::log_status_change(is_offline, true, 0);
                    is_offline = true;
                    is_offline_atomic.store(true, Ordering::Relaxed);
                    if did_check {
                        backoff.on_failure();
                    }

                    offline_buffer.push(m);
                    buffer_len_atomic.store(offline_buffer.len() as u64, Ordering::Relaxed);
                    tracing::warn!(buffered = offline_buffer.len(), max = cfg.buffer_max_size, "offline — buffering metrics");
                }
            }
            _ = &mut shutdown => {
                info!("shutdown signal received, stopping");
                break;
            }
        }
    }
}

// ── Windows Service ───────────────────────────────────────────────────────────

#[cfg(windows)]
mod service {
    use std::ffi::OsString;
    use std::time::Duration;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    const SERVICE_NAME: &str = "OxiPulse";

    define_windows_service!(ffi_service_main, service_main);

    /// Called by the SCM. Blocks until the service stops.
    pub fn start() -> Result<(), windows_service::Error> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
    }

    fn service_main(_arguments: Vec<OsString>) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let shutdown_tx = std::sync::Mutex::new(Some(shutdown_tx));

        let status_handle = service_control_handler::register(
            SERVICE_NAME,
            move |control_event| match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    if let Ok(mut guard) = shutdown_tx.lock() {
                        if let Some(tx) = guard.take() {
                            let _ = tx.send(());
                        }
                    }
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            },
        )
        .expect("failed to register service control handler");

        status_handle
            .set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: ServiceState::Running,
                controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::default(),
                process_id: None,
            })
            .expect("failed to set service status Running");

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");

        rt.block_on(super::run(shutdown_rx));

        let _ = status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        });
    }
}

#[cfg(windows)]
fn run_console() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    rt.block_on(async {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            let _ = shutdown_tx.send(());
        });
        run(shutdown_rx).await;
    });
}

#[cfg(windows)]
fn main() {
    // ERROR_FAILED_SERVICE_CONTROLLER_CONNECT (1063): process was not started
    // by the SCM, so run in console mode instead.
    match service::start() {
        Ok(_) => {}
        Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1063) => {
            run_console();
        }
        Err(e) => {
            eprintln!("[oxipulse] service error: {e}");
            std::process::exit(1);
        }
    }
}

// ── Linux / macOS ─────────────────────────────────────────────────────────────

#[cfg(not(windows))]
#[tokio::main]
async fn main() {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let _ = shutdown_tx.send(());
    });
    run(shutdown_rx).await;
}
