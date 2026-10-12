# Send host metrics to Uptrace

Uptrace receives OTLP/gRPC on port 14317 and identifies the project by the `uptrace-dsn` header.

## 1. Start Uptrace

```bash
git clone https://github.com/uptrace/uptrace
cd uptrace/example/docker
docker compose up -d
```

The UI is at `http://<uptrace-host>:14318` (login `admin@uptrace.local` / `admin` in the example; change it). The example project's DSN is `http://project1_secret@localhost:14318?grpc=14317`. For your own projects, copy the DSN from **Project** → **Settings**.

The example compose also starts an OTel Collector on 4317 that sends the Uptrace machine's own host metrics. OxiPulse does not need it.

## 2. Point OxiPulse at it

In `/etc/oxipulse/config.toml`:

```toml
endpoint = "http://<uptrace-host>:14317"

[headers]
uptrace-dsn = "http://project1_secret@<uptrace-host>:14318?grpc=14317"
```

Then `sudo systemctl restart oxipulse`. With Docker: `-e OXIPULSE_HEADERS="uptrace-dsn=http://project1_secret@<uptrace-host>:14318?grpc=14317"`.

## 3. See the metrics

Uptrace converts names to Prometheus style: `system_cpu_utilization`, `system_memory_usage`, and so on. Open **Metrics** → **Explore** and group by `host_name`.
