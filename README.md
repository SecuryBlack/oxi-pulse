[![Release](https://github.com/securyblack/oxi-pulse/actions/workflows/release.yml/badge.svg)](https://github.com/securyblack/oxi-pulse/actions/workflows/release.yml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![OpenTelemetry](https://img.shields.io/badge/protocol-OpenTelemetry%20OTLP-blueviolet)](https://opentelemetry.io/)

<br />

<p align="center">
  <img src="assets/oxipulse-banner.svg" alt="OxiPulse" width="560" />
</p>

<p align="center">
  <strong>
    <a href="https://github.com/securyblack/oxi-pulse#quickstart">Quickstart</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;
    <a href="https://github.com/securyblack/oxi-pulse#installation">Install</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;
    <a href="https://github.com/securyblack/oxi-pulse#configuration">Configuration</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;
    <a href="https://github.com/securyblack/oxi-pulse/releases/latest">Download</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;
    <a href="https://securyblack.com">SecuryBlack Cloud</a>
  </strong>
</p>

---

## What is OxiPulse?

**OxiPulse** is an ultralight, open source server monitoring agent written in [Rust][urls.rust].
It collects your server's vital signs — CPU, RAM, disk, and network — and ships them
to any [OpenTelemetry][urls.otel]-compatible backend using the OTLP protocol.

OxiPulse is designed to be the monitoring agent you actually want to run:

- **No bloat.** A single static binary with near-zero CPU and memory overhead.
- **No lock-in.** Send metrics to [SecuryBlack Cloud][urls.securyblack], your own self-hosted
  collector, Grafana Cloud, Datadog, or any OTLP endpoint — your choice.
- **No downtime on updates.** The agent updates itself daily from GitHub Releases
  and restarts cleanly through your service manager.
- **No data loss.** An offline buffer survives network outages and flushes
  automatically when the connection is restored.

OxiPulse is maintained by [SecuryBlack][urls.securyblack]'s engineering team as a
fully open source project under the Apache 2.0 license.

### Principles

- **Ultralight** — Built in [Rust][urls.rust]. Designed to run invisibly on every server,
  from a Raspberry Pi to a 256-core bare metal machine.
- **Standard protocol** — Uses [OpenTelemetry OTLP][urls.otel], the industry standard for
  telemetry. No proprietary wire formats.
- **Vendor-neutral** — The agent has no hardcoded backend. Point it anywhere.
- **Resilient** — Exponential backoff and an in-memory ring buffer ensure no data
  is dropped during outages, up to 24 hours of buffering by default.
- **Self-updating** — Automatically pulls the correct binary for your platform
  from GitHub Releases once per day.

### Use cases

- Monitor bare-metal servers, VMs, and cloud instances from a single pane of glass.
- Send metrics to your existing OpenTelemetry stack (Grafana, Jaeger, Datadog, etc.).
- Use [SecuryBlack Cloud][urls.securyblack] for a zero-config hosted monitoring experience.
- Embed OxiPulse into your own product as a lightweight telemetry foundation.

---

## Quickstart

### Linux — one command

```bash
curl -fsSL https://install.oxipulse.io | bash
```

The script will ask for your OTLP endpoint and auth token, then install the agent
as a **systemd service** with automatic restart.

### Windows — one command (PowerShell as Administrator)

```powershell
irm https://install.oxipulse.io/windows | iex
```

The script installs the agent as a **Windows Service** with automatic restart on failure.

### Using SecuryBlack Cloud

Sign in to [app.securyblack.com][urls.securyblack_app], add a server, and copy the
pre-filled install command — your endpoint and token are already included:

```bash
# Linux
curl -fsSL https://install.oxipulse.io | bash -s -- \
  --endpoint ingest.securyblack.com \
  --token <YOUR_TOKEN>

# Windows
irm https://install.oxipulse.io/windows | iex -Endpoint ingest.securyblack.com -Token <YOUR_TOKEN>
```

---

## Installation

### Pre-built binaries

Download the latest binary for your platform from the [Releases page][urls.releases].

| Platform          | Asset                                        |
|-------------------|----------------------------------------------|
| Linux x86\_64     | `oxipulse-x86_64-unknown-linux-gnu.tar.gz`   |
| Linux arm64       | `oxipulse-aarch64-unknown-linux-gnu.tar.gz`  |
| Windows x86\_64   | `oxipulse-x86_64-pc-windows-msvc.zip`        |

### Build from source

```bash
git clone https://github.com/securyblack/oxi-pulse
cd oxi-pulse
cargo build --release
```

Requires Rust 1.70+.

---

## Configuration

OxiPulse is configured via `/etc/oxipulse/config.toml` (Linux) or
`C:\ProgramData\oxipulse\config.toml` (Windows), with environment variables
as overrides.

```toml
# Required
endpoint = "https://ingest.example.com:4317"   # Any OTLP/gRPC endpoint
token    = "your-auth-token"                    # Sent as Authorization: Bearer header

# Optional
interval_secs   = 10    # Collection and export interval (default: 10s)
buffer_max_size = 8640  # Max buffered snapshots when offline (default: 8640 = 24h at 10s)
```

### Environment variable overrides

| Variable                 | Description                     |
|--------------------------|---------------------------------|
| `OXIPULSE_ENDPOINT`      | OTLP collector endpoint URL     |
| `OXIPULSE_TOKEN`         | Auth token                      |
| `OXIPULSE_INTERVAL_SECS` | Collection interval in seconds  |
| `OXIPULSE_BUFFER_MAX`    | Max offline buffer size         |

### Logging

Control log verbosity via the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug oxipulse        # Verbose
RUST_LOG=warn  oxipulse        # Warnings and errors only
```

---

## Metrics collected

| Metric                     | OTel Instrument     | Unit |
|----------------------------|---------------------|------|
| `system.cpu.usage`         | Gauge               | %    |
| `system.memory.used`       | Gauge               | By   |
| `system.memory.total`      | Gauge               | By   |
| `system.disk.used`         | Gauge               | By   |
| `system.disk.total`        | Gauge               | By   |
| `system.network.received`  | Counter (delta)     | By   |
| `system.network.transmitted` | Counter (delta)   | By   |

All metrics use standard [OpenTelemetry semantic conventions][urls.otel_semconv].

---

## Comparison

| Feature                      | **OxiPulse**  | Telegraf  | Prometheus Node Exporter | Datadog Agent | New Relic Agent |
|------------------------------|:-------------:|:---------:|:------------------------:|:-------------:|:---------------:|
| Written in Rust              | **✓**         |           |                          |               |                 |
| Single static binary         | **✓**         | ✓         | ✓                        |               |                 |
| OpenTelemetry OTLP native    | **✓**         | ⚠         |                          | ⚠             |                 |
| Vendor-neutral               | **✓**         | ✓         | ✓                        |               |                 |
| Self-updating                | **✓**         |           |                          | ✓             | ✓               |
| Offline buffer               | **✓**         | ⚠         |                          | ✓             | ✓               |
| Memory-safe                  | **✓**         |           |                          |               |                 |
| Open source (Apache 2.0)     | **✓**         | ✓         | ✓                        |               |                 |
| <10 MB binary                | **✓**         |           | ✓                        |               |                 |

⚠ = Partial or requires additional configuration

---

## Service management

### Linux (systemd)

```bash
# Status
systemctl status oxipulse

# Live logs
journalctl -fu oxipulse

# Restart
systemctl restart oxipulse

# Uninstall
systemctl disable --now oxipulse && rm /usr/local/bin/oxipulse /etc/systemd/system/oxipulse.service
```

### Windows (PowerShell)

```powershell
# Status
Get-Service OxiPulse

# Logs (Event Viewer)
Get-EventLog -LogName Application -Source OxiPulse -Newest 50

# Restart
Restart-Service OxiPulse

# Uninstall
Stop-Service OxiPulse; sc.exe delete OxiPulse
```

---

## Contributing

OxiPulse is open source and contributions are welcome.

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Commit your changes following [Conventional Commits][urls.conventional_commits]
4. Open a pull request

Please read our [Code of Conduct][urls.code_of_conduct] before contributing.

### Development

```bash
# Run with a local OTLP collector (e.g. otelcol)
OXIPULSE_ENDPOINT=http://localhost:4317 OXIPULSE_TOKEN=dev cargo run

# Build for all platforms (requires WSL2 on Windows for Linux targets)
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-pc-windows-msvc
```

---

## Releases

OxiPulse follows [Semantic Versioning][urls.semver]. Every push to a `v*` tag
triggers the CI/CD pipeline, which cross-compiles binaries for all supported
platforms and publishes them to [GitHub Releases][urls.releases] with SHA256 checksums.

The agent's self-update mechanism checks for new releases every 24 hours and
applies updates automatically. No manual intervention required.

---

## Security

If you discover a security vulnerability, please follow our
[Security Policy][urls.security_policy] and report it privately.
**Do not open a public GitHub issue for security vulnerabilities.**

---

## License

OxiPulse is licensed under the [Apache License, Version 2.0][urls.license].

All dependencies are compatible with Apache 2.0 (MIT, Apache-2.0, or BSD licensed).

---

<p align="center">
  Developed with care by <strong><a href="https://securyblack.com">SecuryBlack</a></strong>
  &nbsp;&mdash;&nbsp;
  <a href="https://github.com/securyblack/oxi-pulse/security/policy">Security Policy</a>
  &nbsp;&mdash;&nbsp;
  <a href="https://github.com/securyblack/oxi-pulse/blob/main/LICENSE">Apache 2.0 License</a>
</p>

[urls.rust]: https://www.rust-lang.org/
[urls.otel]: https://opentelemetry.io/
[urls.otel_semconv]: https://opentelemetry.io/docs/specs/semconv/system/
[urls.securyblack]: https://securyblack.com
[urls.securyblack_app]: https://app.securyblack.com
[urls.releases]: https://github.com/securyblack/oxi-pulse/releases/latest
[urls.security_policy]: https://github.com/securyblack/oxi-pulse/security/policy
[urls.license]: https://github.com/securyblack/oxi-pulse/blob/main/LICENSE
[urls.code_of_conduct]: https://github.com/securyblack/oxi-pulse/blob/main/CODE_OF_CONDUCT.md
[urls.conventional_commits]: https://www.conventionalcommits.org/
[urls.semver]: https://semver.org/
