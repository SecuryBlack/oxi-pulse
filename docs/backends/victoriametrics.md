# Send host metrics to VictoriaMetrics

VictoriaMetrics takes OTLP over HTTP only, and OxiPulse sends OTLP/gRPC, so an OpenTelemetry Collector sits in between. The [example](../../examples/victoriametrics) runs both.

## 1. Start VictoriaMetrics and the Collector

```bash
git clone https://github.com/SecuryBlack/oxi-pulse
cd oxi-pulse/examples/victoriametrics
docker compose up -d
```

- The Collector listens for OxiPulse on 4317 and forwards to VictoriaMetrics over HTTP ([otel-collector.yaml](../../examples/victoriametrics/otel-collector.yaml)).
- VictoriaMetrics runs with `-opentelemetry.usePrometheusNaming`, so names match the Prometheus setup (`system_cpu_utilization_ratio`) and the [Grafana dashboard](../../examples/prometheus-grafana/grafana/oxipulse-hosts.json) works unchanged.

Already running VictoriaMetrics? Copy the Collector service and add that flag to your instance.

## 2. Point OxiPulse at it

```bash
curl -fsSL https://install.oxipulse.dev | sudo bash -s -- --endpoint http://<collector-host>:4317
```

Port 4317 has no auth or TLS here: keep it on a private network or VPN.

## 3. See the metrics

```bash
curl 'http://<vm-host>:8428/api/v1/query?query=system_cpu_utilization_ratio'
```

Every series has a `host_name` label. In Grafana, add VictoriaMetrics as a Prometheus data source (`http://<vm-host>:8428`) and import the OxiPulse dashboard. The PromQL examples in the [Prometheus guide](../../examples/prometheus-grafana) work as they are.
