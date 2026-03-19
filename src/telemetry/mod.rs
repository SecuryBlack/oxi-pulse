use opentelemetry::{
    global,
    metrics::{Counter, Gauge, MeterProvider as _},
    KeyValue,
};
use opentelemetry_otlp::{MetricExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    metrics::{PeriodicReader, SdkMeterProvider},
    runtime,
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
    net_bytes_in: Counter<u64>,
    net_bytes_out: Counter<u64>,
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
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
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
        net_bytes_in: meter
            .u64_counter("system.network.received")
            .with_description("Network bytes received since last interval")
            .with_unit("By")
            .build(),
        net_bytes_out: meter
            .u64_counter("system.network.transmitted")
            .with_description("Network bytes transmitted since last interval")
            .with_unit("By")
            .build(),
    };

    Ok((instruments, provider))
}

/// Record a collected `Metrics` snapshot into the OTel instruments.
pub fn record(instruments: &Instruments, m: &Metrics) {
    let attrs: &[KeyValue] = &[];
    instruments.cpu_usage.record(m.cpu_usage_percent as f64, attrs);
    instruments.ram_used.record(m.ram_used_bytes, attrs);
    instruments.ram_total.record(m.ram_total_bytes, attrs);
    instruments.disk_used.record(m.disk_used_bytes, attrs);
    instruments.disk_total.record(m.disk_total_bytes, attrs);
    instruments.net_bytes_in.add(m.net_bytes_in, attrs);
    instruments.net_bytes_out.add(m.net_bytes_out, attrs);
}
