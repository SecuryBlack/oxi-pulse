use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Gauge, MeterProvider as _},
};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Duration;
use tonic::metadata::{MetadataMap, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig};

mod exporter;

use crate::metrics::Metrics;

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
    token: &str,
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

    // Build metadata map with the auth token (omitted for collectors without auth)
    let mut metadata = MetadataMap::new();
    if !token.is_empty() {
        let auth_value = MetadataValue::try_from(format!("Bearer {}", token))?;
        metadata.insert("authorization", auth_value);
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
}
