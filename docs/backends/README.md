# Backend guides

OxiPulse sends host metrics over OTLP/gRPC. Each guide below starts the backend, points OxiPulse at it and shows a query that proves the metrics arrived. The OpenObserve, Uptrace and VictoriaMetrics setups are tested end to end in CI ([backends.yml](../../.github/workflows/backends.yml)) on every change.

| Backend | How OxiPulse reaches it | Guide |
|---|---|---|
| SigNoz | Directly (gRPC 4317); not yet tested in CI | [signoz.md](signoz.md) |
| OpenObserve | Directly (gRPC 5081, Basic auth headers) | [openobserve.md](openobserve.md) |
| Uptrace | Directly (gRPC 14317, `uptrace-dsn` header) | [uptrace.md](uptrace.md) |
| VictoriaMetrics | Through an OTel Collector (VictoriaMetrics takes OTLP over HTTP only) | [victoriametrics.md](victoriametrics.md) |
| Prometheus + Grafana | Through an OTel Collector | [examples/prometheus-grafana](../../examples/prometheus-grafana) |

## Headers

Backends that want their own auth or routing headers get them through `headers`, as a TOML table or as `OXIPULSE_HEADERS` (`key=value,key=value`, the format of `OTEL_EXPORTER_OTLP_HEADERS`):

```toml
endpoint = "https://ingest.eu.signoz.cloud:443"

[headers]
signoz-ingestion-key = "<your key>"
```

`token` is still sent as `authorization: Bearer <token>`, unless `headers` sets its own `authorization`. `[headers]` must go after the other settings in `config.toml`.
