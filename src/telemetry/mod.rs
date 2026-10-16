use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Gauge, MeterProvider as _},
};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tonic::metadata::{MetadataMap, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig};

mod exporter;

use crate::metrics::{Metrics, ServiceUsage};

// Metric names follow the OpenTelemetry system semantic conventions, so the
// same queries and dashboards work for OxiPulse and the Collector's hostmetrics
// receiver. Usage metrics are gauges (the spec's UpDownCounter maps to the same
// Prometheus series); system.network.latency has no semantic convention.
pub struct Instruments {
    uptime: Gauge<u64>,
    cpu_count: Gauge<u64>,
    cpu_utilization: Gauge<f64>,
    load_avg_1m: Gauge<f64>,
    load_avg_5m: Gauge<f64>,
    load_avg_15m: Gauge<f64>,
    memory_usage: Gauge<u64>,
    memory_limit: Gauge<u64>,
    paging_usage: Gauge<u64>,
    filesystem_usage: Gauge<u64>,
    network_io: Counter<u64>,
    net_latency: Gauge<f64>,
    filesystem_inodes: Gauge<u64>,
    cpu_mode_utilization: Gauge<f64>,
    pressure: Gauge<f64>,
    oom_kills: Gauge<u64>,
    failed_units: Gauge<u64>,
    threads: Gauge<u64>,
    /// Read by observable gauges, which always export with delta temporality:
    /// a service that leaves the top is not exported again with a stale value,
    /// as a synchronous gauge with cumulative temporality would.
    services: Arc<Mutex<Vec<ServiceUsage>>>,
}

#[derive(Debug, Clone, Default)]
pub struct HostInfo {
    pub host_name: Option<String>,
    pub cpu_model: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
}

/// Initialise the OTLP metrics pipeline and return the instruments to record into.
/// The returned `SdkMeterProvider` must be kept alive for the duration of the process.
pub fn init(
    endpoint: &str,
    headers: &BTreeMap<String, String>,
    interval_secs: u64,
    buffer_max: usize,
    queued: Arc<AtomicU64>,
    offline: Arc<AtomicBool>,
    host_info: HostInfo,
) -> Result<(Instruments, SdkMeterProvider), Box<dyn std::error::Error>> {
    let normalized_endpoint =
        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            format!("https://{}", endpoint)
        } else {
            endpoint.to_string()
        };

    // gRPC metadata: the Bearer token (if any) plus the user's own headers
    let mut metadata = MetadataMap::new();
    for (key, value) in headers {
        let key = tonic::metadata::AsciiMetadataKey::from_bytes(key.as_bytes())?;
        metadata.insert(key, MetadataValue::try_from(value.as_str())?);
    }

    // OTLP/gRPC exporter that keeps what the backend did not accept
    let mut channel = Channel::from_shared(normalized_endpoint.clone())?
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10));
    if normalized_endpoint.starts_with("https://") {
        channel = channel.tls_config(ClientTlsConfig::new().with_native_roots())?;
    }
    let exporter = exporter::QueueExporter::new(
        channel.connect_lazy(),
        metadata,
        buffer_max,
        queued,
        offline,
    );

    // Periodic reader flushes on the same interval as our collection loop
    let reader = PeriodicReader::builder(exporter)
        .with_interval(Duration::from_secs(interval_secs))
        .build();

    // Build and register the global MeterProvider
    let mut attrs = vec![
        KeyValue::new("service.name", "oxipulse"),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ];
    if let Some(name) = host_info.host_name {
        attrs.push(KeyValue::new("host.name", name));
    }
    if let Some(cpu) = host_info.cpu_model {
        attrs.push(KeyValue::new("host.cpu.model", cpu));
    }
    if let Some(os) = host_info.os_name {
        attrs.push(KeyValue::new("host.os.name", os));
    }
    if let Some(ver) = host_info.os_version {
        attrs.push(KeyValue::new("host.os.version", ver));
    }
    if let Some(kver) = host_info.kernel_version {
        attrs.push(KeyValue::new("host.kernel.version", kver));
    }

    let resource = Resource::builder().with_attributes(attrs).build();

    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(resource)
        .build();
    global::set_meter_provider(provider.clone());

    // Create instruments under the "oxipulse" meter
    let meter = provider.meter("oxipulse");

    let services: Arc<Mutex<Vec<ServiceUsage>>> = Arc::default();
    let snapshot = services.clone();
    meter
        .u64_observable_gauge("system.service.memory.usage")
        .with_description("Memory used by a systemd service (cgroup), top services only")
        .with_unit("By")
        .with_callback(move |o| {
            for s in snapshot.lock().unwrap().iter() {
                o.observe(
                    s.memory_bytes,
                    &[KeyValue::new("service.unit", s.unit.clone())],
                );
            }
        })
        .build();
    let snapshot = services.clone();
    meter
        .f64_observable_gauge("system.service.cpu.utilization")
        .with_description(
            "CPU used by a systemd service as a fraction of all CPUs (0-1), top services only",
        )
        .with_unit("1")
        .with_callback(move |o| {
            for s in snapshot.lock().unwrap().iter() {
                o.observe(
                    s.cpu_utilization,
                    &[KeyValue::new("service.unit", s.unit.clone())],
                );
            }
        })
        .build();

    let instruments = Instruments {
        uptime: meter
            .u64_gauge("system.uptime")
            .with_description("Time the system has been running")
            .with_unit("s")
            .build(),
        cpu_count: meter
            .u64_gauge("system.cpu.logical.count")
            .with_description("Number of logical CPUs")
            .with_unit("{cpu}")
            .build(),
        cpu_utilization: meter
            .f64_gauge("system.cpu.utilization")
            .with_description("CPU busy time as a fraction of all CPUs (0-1)")
            .with_unit("1")
            .build(),
        load_avg_1m: meter
            .f64_gauge("system.cpu.load_average.1m")
            .with_description("Load average over 1 minute")
            .with_unit("{thread}")
            .build(),
        load_avg_5m: meter
            .f64_gauge("system.cpu.load_average.5m")
            .with_description("Load average over 5 minutes")
            .with_unit("{thread}")
            .build(),
        load_avg_15m: meter
            .f64_gauge("system.cpu.load_average.15m")
            .with_description("Load average over 15 minutes")
            .with_unit("{thread}")
            .build(),
        memory_usage: meter
            .u64_gauge("system.memory.usage")
            .with_description("Memory in use and free, by state")
            .with_unit("By")
            .build(),
        memory_limit: meter
            .u64_gauge("system.memory.limit")
            .with_description("Total memory")
            .with_unit("By")
            .build(),
        paging_usage: meter
            .u64_gauge("system.paging.usage")
            .with_description("Swap in use and free, by state")
            .with_unit("By")
            .build(),
        filesystem_usage: meter
            .u64_gauge("system.filesystem.usage")
            .with_description("Filesystem space in use and free, by state")
            .with_unit("By")
            .build(),
        network_io: meter
            .u64_counter("system.network.io")
            .with_description("Bytes received and transmitted, per interface")
            .with_unit("By")
            .build(),
        net_latency: meter
            .f64_gauge("system.network.latency")
            .with_description("TCP connect time to a target, -1 on failure")
            .with_unit("ms")
            .build(),
        filesystem_inodes: meter
            .u64_gauge("system.filesystem.inodes.usage")
            .with_description("Filesystem inodes in use and free, by state")
            .with_unit("{inode}")
            .build(),
        // Linux only, from here down.
        cpu_mode_utilization: meter
            .f64_gauge("system.cpu.mode.utilization")
            .with_description("CPU time in steal and iowait as a fraction of all CPUs (0-1)")
            .with_unit("1")
            .build(),
        pressure: meter
            .f64_gauge("system.pressure.stall")
            .with_description("Fraction of time tasks were stalled on cpu, memory or io since the previous collection (PSI)")
            .with_unit("1")
            .build(),
        oom_kills: meter
            .u64_gauge("system.oom_kills")
            .with_description("Processes killed by the OOM killer since the previous collection")
            .with_unit("{process}")
            .build(),
        failed_units: meter
            .u64_gauge("system.systemd.units.failed")
            .with_description("systemd units in the failed state")
            .with_unit("{unit}")
            .build(),
        threads: meter
            .u64_gauge("system.thread.count")
            .with_description("Threads on the system")
            .with_unit("{thread}")
            .build(),
        services,
    };

    Ok((instruments, provider))
}

/// Record a collected `Metrics` snapshot into the OTel instruments.
pub fn record(instruments: &Instruments, m: &Metrics) {
    for disk in &m.disks {
        let free = disk.total_bytes.saturating_sub(disk.used_bytes);
        for (state, value) in [("used", disk.used_bytes), ("free", free)] {
            instruments.filesystem_usage.record(
                value,
                &[
                    KeyValue::new("device", disk.name.clone()),
                    KeyValue::new("mountpoint", disk.mount_point.clone()),
                    KeyValue::new("type", disk.fs_type.clone()),
                    KeyValue::new("state", state),
                ],
            );
        }
        if let Some((total, used)) = disk.inodes {
            for (state, value) in [("used", used), ("free", total.saturating_sub(used))] {
                instruments.filesystem_inodes.record(
                    value,
                    &[
                        KeyValue::new("device", disk.name.clone()),
                        KeyValue::new("mountpoint", disk.mount_point.clone()),
                        KeyValue::new("state", state),
                    ],
                );
            }
        }
    }
    for n in &m.networks {
        for (direction, value) in [("receive", n.received), ("transmit", n.transmitted)] {
            instruments.network_io.add(
                value,
                &[
                    KeyValue::new("device", n.device.clone()),
                    KeyValue::new("direction", direction),
                ],
            );
        }
    }
    instruments.uptime.record(m.uptime_secs, &[]);
    instruments.cpu_count.record(m.cpu_count as u64, &[]);
    instruments
        .cpu_utilization
        .record(m.cpu_usage_percent as f64 / 100.0, &[]);
    // sysinfo has no load average on Windows.
    if !cfg!(windows) {
        instruments.load_avg_1m.record(m.load_avg_1m, &[]);
        instruments.load_avg_5m.record(m.load_avg_5m, &[]);
        instruments.load_avg_15m.record(m.load_avg_15m, &[]);
    }
    let ram_free = m.ram_total_bytes.saturating_sub(m.ram_used_bytes);
    for (state, value) in [("used", m.ram_used_bytes), ("free", ram_free)] {
        instruments
            .memory_usage
            .record(value, &[KeyValue::new("state", state)]);
    }
    instruments.memory_limit.record(m.ram_total_bytes, &[]);
    if m.swap_total_bytes > 0 {
        let swap_free = m.swap_total_bytes.saturating_sub(m.swap_used_bytes);
        for (state, value) in [("used", m.swap_used_bytes), ("free", swap_free)] {
            instruments
                .paging_usage
                .record(value, &[KeyValue::new("state", state)]);
        }
    }

    for lat in &m.latencies {
        let attrs: &[KeyValue] = &[
            KeyValue::new("target", lat.target.clone()),
            KeyValue::new(
                "status",
                if lat.latency_ms.is_some() {
                    "success"
                } else {
                    "failure"
                },
            ),
        ];
        let val = lat.latency_ms.unwrap_or(-1.0);
        instruments.net_latency.record(val, attrs);
    }

    let l = &m.linux;
    for (mode, value) in [("steal", l.cpu_steal), ("iowait", l.cpu_iowait)] {
        if let Some(v) = value {
            instruments
                .cpu_mode_utilization
                .record(v, &[KeyValue::new("cpu.mode", mode)]);
        }
    }
    for p in &l.pressure {
        instruments.pressure.record(
            p.value,
            &[
                KeyValue::new("resource", p.resource),
                KeyValue::new("kind", p.kind),
            ],
        );
    }
    if let Some(v) = l.oom_kills {
        instruments.oom_kills.record(v, &[]);
    }
    if let Some(v) = l.failed_units {
        instruments.failed_units.record(v, &[]);
    }
    if let Some(v) = l.threads {
        instruments.threads.record(v, &[]);
    }
    *instruments.services.lock().unwrap() = l.services.clone();
}
