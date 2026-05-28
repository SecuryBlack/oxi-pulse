use opentelemetry::{
    global,
    metrics::{Gauge, MeterProvider as _},
    KeyValue,
};
use opentelemetry_otlp::{MetricExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    metrics::{PeriodicReader, SdkMeterProvider},
    runtime, Resource,
};
use std::time::Duration;
use tonic::metadata::{MetadataMap, MetadataValue};
use tonic::transport::ClientTlsConfig;

use crate::metrics::Metrics;

pub struct Instruments {
    cpu_usage: Gauge<f64>,
    ram_used: Gauge<u64>,
    ram_total: Gauge<u64>,
    disk_used: Gauge<u64>,
    disk_total: Gauge<u64>,
    net_bps_in: Gauge<f64>,
    net_bps_out: Gauge<f64>,
    net_latency: Gauge<f64>,
}

/// Initialise the OTLP metrics pipeline and return the instruments to record into.
/// The returned `SdkMeterProvider` must be kept alive for the duration of the process.
pub fn init(
    endpoint: &str,
    token: &str,
    interval_secs: u64,
) -> Result<(Instruments, SdkMeterProvider), Box<dyn std::error::Error>> {
    // Build metadata map with the auth token
    let mut metadata = MetadataMap::new();
    let auth_value = MetadataValue::try_from(format!("Bearer {}", token))?;
    metadata.insert("authorization", auth_value);

    // Build the OTLP exporter
    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .with_metadata(metadata)
        .with_tls_config(ClientTlsConfig::new().with_native_roots())
        .build()?;

    // Periodic reader flushes on the same interval as our collection loop
    let reader = PeriodicReader::builder(exporter, runtime::Tokio)
        .with_interval(Duration::from_secs(interval_secs))
        .build();

    // Build and register the global MeterProvider
    let resource = Resource::new(vec![
        KeyValue::new("service.name", "oxipulse"),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ]);

    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(resource)
        .build();
    global::set_meter_provider(provider.clone());

    // Create instruments under the "oxipulse" meter
    let meter = provider.meter("oxipulse");

    let instruments = Instruments {
        cpu_usage: meter
            .f64_gauge("system.cpu.usage")
            .with_description("CPU usage percentage (0-100)")
            .with_unit("%")
            .build(),
        ram_used: meter
            .u64_gauge("system.memory.used")
            .with_description("Used RAM in bytes")
            .with_unit("By")
            .build(),
        ram_total: meter
            .u64_gauge("system.memory.total")
            .with_description("Total RAM in bytes")
            .with_unit("By")
            .build(),
        disk_used: meter
            .u64_gauge("system.disk.used")
            .with_description("Used disk space in bytes")
            .with_unit("By")
            .build(),
        disk_total: meter
            .u64_gauge("system.disk.total")
            .with_description("Total disk space in bytes")
            .with_unit("By")
            .build(),
        net_bps_in: meter
            .f64_gauge("system.network.receive")
            .with_description("Network receive throughput in bytes per second")
            .with_unit("By/s")
            .build(),
        net_bps_out: meter
            .f64_gauge("system.network.transmit")
            .with_description("Network transmit throughput in bytes per second")
            .with_unit("By/s")
            .build(),
        net_latency: meter
            .f64_gauge("system.network.latency")
            .with_description("Network latency to target in milliseconds")
            .with_unit("ms")
            .build(),
    };

    Ok((instruments, provider))
}

/// Record a collected `Metrics` snapshot into the OTel instruments.
pub fn record(instruments: &Instruments, m: &Metrics) {
    for disk in &m.disks {
        let attrs: &[KeyValue] = &[KeyValue::new("disk.name", disk.name.clone())];
        instruments
            .disk_used
            .record(disk.used_bytes, attrs);
        instruments
            .disk_total
            .record(disk.total_bytes, attrs);
    }
    instruments.cpu_usage.record(m.cpu_usage_percent as f64, &[]);
    instruments.ram_used.record(m.ram_used_bytes, &[]);
    instruments.ram_total.record(m.ram_total_bytes, &[]);
    instruments.net_bps_in.record(m.net_bps_in, &[]);
    instruments.net_bps_out.record(m.net_bps_out, &[]);

    for lat in &m.latencies {
        let attrs: &[KeyValue] = &[
            KeyValue::new("target", lat.target.clone()),
            KeyValue::new("status", if lat.latency_ms.is_some() { "success" } else { "failure" }),
        ];
        let val = lat.latency_ms.unwrap_or(-1.0);
        instruments.net_latency.record(val, attrs);
    }
}
