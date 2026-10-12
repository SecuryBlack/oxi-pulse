# Send host metrics to SigNoz

SigNoz receives OTLP/gRPC on port 4317 without auth, so OxiPulse talks to it directly, as it does with any OpenTelemetry Collector.

This guide is not yet tested in our CI, unlike the others: in a fresh SigNoz install from Foundry 0.3.0 on GitHub Actions, the ingester never opened port 4317, so nothing could send to it. If the OTLP receiver does not answer, check `docker logs signoz-ingester-1`.

## 1. Start SigNoz

SigNoz installs with [Foundry](https://signoz.io/docs/install/docker/). On the machine that will run it:

```bash
curl -fsSL https://signoz.io/foundry.sh | bash
cat > casting.yaml <<'YAML'
apiVersion: v1alpha1
kind: Installation
metadata:
  name: signoz
spec:
  deployment:
    flavor: compose
    mode: docker
YAML
foundryctl cast -f casting.yaml
```

The UI is at `http://<signoz-host>:8080` and the OTLP/gRPC receiver at port 4317.

## 2. Point OxiPulse at it

```bash
curl -fsSL https://install.oxipulse.dev | sudo bash -s -- --endpoint http://<signoz-host>:4317
```

Port 4317 has no auth or TLS here: keep it on a private network or VPN.

## 3. See the metrics

In SigNoz, open **Metrics** → **Explorer** and search for `system.cpu.utilization`. Group by `host.name` to get one line per server. All metric names are listed in the [README](../../README.md#metrics).

To check from the command line:

```bash
docker exec signoz-telemetrystore-clickhouse-0-0 clickhouse-client -q \
  "SELECT count() FROM signoz_metrics.distributed_samples_v4 WHERE metric_name='system.cpu.utilization'"
```

## SigNoz Cloud

Use your region's ingest endpoint and the ingestion key as a header (from SigNoz's [docs](https://signoz.io/docs/ingestion/signoz-cloud/overview/); not covered by our CI):

```toml
endpoint = "https://ingest.<region>.signoz.cloud:443"

[headers]
signoz-ingestion-key = "<your ingestion key>"
```
