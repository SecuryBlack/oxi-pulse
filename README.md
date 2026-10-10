# OxiPulse

**A small Rust agent that sends your servers' host metrics to any OpenTelemetry (OTLP) backend.**

One binary (about 4 MB download), installed as a systemd or Windows service. It collects CPU, memory, swap, disk, network, load average and uptime, and pushes them over OTLP/gRPC to your own collector, Grafana/Prometheus, any OTLP-compatible vendor, or [SecuryBlack Cloud][urls.securyblack].

[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/securyblack/oxi-pulse)](https://github.com/securyblack/oxi-pulse/releases/latest)
[![Arch](https://img.shields.io/badge/arch-x86__64%20%7C%20aarch64-informational?logo=linux&logoColor=white)](https://github.com/securyblack/oxi-pulse/releases)
[![OpenTelemetry](https://img.shields.io/badge/protocol-OTLP%2FgRPC-blueviolet)](https://opentelemetry.io/)
[![Website](https://img.shields.io/badge/website-oxipulse.dev-33E1BF)](https://oxipulse.dev)

<p align="center">
  <img src="assets/oxipulse-banner.png" alt="OxiPulse" width="560" />
</p>

- **Light.** On Linux, 11 MB of RSS (3.5 MB private) and 0.01 % of one CPU core at the default 30 s interval; node_exporter used 24 MB and the OTel Collector's hostmetrics receiver 204 MB in the same run. On Windows 11, 15.5 MB of private memory and 0.07 % of one core. Method and reproducible script in [bench/](bench/README.md).
- **Vendor-neutral.** Standard OTLP. No account needed, no hardcoded backend.
- **Offline buffer.** Keeps up to 72 h of metrics (default) while the backend is unreachable and sends them, with their original timestamps, when it comes back. About 2-3 KB per interval in memory, so roughly 20 MB after a full 72 h outage.
- **Self-updating, optionally.** Checks GitHub Releases daily; turn it off with `auto_update = false`.
- **Private by default.** With a non-SecuryBlack endpoint it talks only to your endpoint and GitHub. See [what it sends](#what-oxipulse-sends-and-where).

---

## Quickstart: your own Prometheus + Grafana

On the machine that will store the metrics (needs Docker):

```bash
git clone https://github.com/securyblack/oxi-pulse && cd oxi-pulse/examples/prometheus-grafana
docker compose up -d
```

On every Linux host you want to monitor:

```bash
curl -fsSL https://install.oxipulse.dev | sudo bash -s -- --endpoint http://<collector-host>:4317
```

Open Grafana on `http://<collector-host>:3000`: the home page is a ready-made OxiPulse dashboard with every host. Details in [examples/prometheus-grafana](examples/prometheus-grafana/README.md).

Prefer to read the installer first? It is [scripts/install.sh](scripts/install.sh) (and [install.ps1](scripts/install.ps1) for Windows). You can also download the binary from [Releases][urls.releases] and run it yourself.

---

## Where to send metrics

| Backend | How |
|---|---|
| **Your own stack** (OTel Collector, Prometheus, Grafana, VictoriaMetrics, SigNoz, OpenObserve, Uptrace…) | Point `endpoint` at its OTLP/gRPC receiver (usually port 4317). Set `token` only if it requires a Bearer token |
| **An OTLP vendor** | Their OTLP/gRPC endpoint and API token |
| **[SecuryBlack Cloud][urls.securyblack]** | Hosted dashboards, alerts and security audit for your servers, with a free Hobby plan for one server. Add a server in the [panel][urls.securyblack_app] and copy the install command, which already includes endpoint and token |

---

## Installation

### Linux

```bash
curl -fsSL https://install.oxipulse.dev | sudo bash -s -- --endpoint <OTLP endpoint> [--token <TOKEN>] [--no-auto-update]
```

Without arguments the script asks interactively. It downloads the release for your architecture, verifies its SHA-256 checksum, writes `/etc/oxipulse/config.toml` and installs a systemd service.

### Debian, Ubuntu (.deb) and RHEL, Rocky, Fedora (.rpm)

Each [release][urls.releases] has packages for amd64 and arm64, if you prefer your package manager to a script. Replace the version with the latest one:

```bash
# Debian / Ubuntu
curl -fsSLO https://github.com/securyblack/oxi-pulse/releases/download/v0.4.1/oxipulse_0.4.1_amd64.deb
sudo apt install ./oxipulse_0.4.1_amd64.deb

# RHEL / Rocky / Alma / Fedora
sudo dnf install https://github.com/securyblack/oxi-pulse/releases/download/v0.4.1/oxipulse-0.4.1-1.x86_64.rpm
```

The package installs `/usr/bin/oxipulse` and a systemd service but does not start it. Set `endpoint` (and `token` if needed) in `/etc/oxipulse/config.toml`, then `sudo systemctl enable --now oxipulse`. Self-update is off; upgrades come from installing the next package, which keeps your config.

### Docker

Multi-arch image (amd64, arm64) on GitHub Container Registry. Run it with the host network, so it sees the host's interfaces and hostname, and the host's `/` mounted read-only, so it sees the host's disks:

```bash
docker run -d --name oxipulse --restart unless-stopped   --network host   -v /:/hostfs:ro   -e OXIPULSE_HOST_ROOT=/hostfs   -e OXIPULSE_ENDPOINT=http://<collector-host>:4317   ghcr.io/securyblack/oxipulse:latest
```

Or with Compose:

```yaml
services:
  oxipulse:
    image: ghcr.io/securyblack/oxipulse:latest
    network_mode: host
    volumes:
      - /:/hostfs:ro
    environment:
      OXIPULSE_HOST_ROOT: /hostfs
      OXIPULSE_ENDPOINT: http://<collector-host>:4317
      # OXIPULSE_TOKEN: <token>
    restart: unless-stopped
```

All [configuration](#configuration) goes through `OXIPULSE_*` variables. The container runs as a non-root user, logs to stdout (`docker logs oxipulse`) and does not update itself: pull a new tag instead. Pin a version (`ghcr.io/securyblack/oxipulse:0.4.1`) if you prefer.

### Windows (PowerShell as Administrator)

```powershell
$script = irm https://install.oxipulse.dev
& ([scriptblock]::Create($script)) -Endpoint "https://collector.example.com:4317"
```

`-Token "<TOKEN>"` and `-NoAutoUpdate` are optional.

### Pre-built binaries

| Platform          | Asset                                        |
|-------------------|----------------------------------------------|
| Linux x86\_64     | `oxipulse-x86_64-unknown-linux-gnu.tar.gz`   |
| Linux arm64       | `oxipulse-aarch64-unknown-linux-gnu.tar.gz`  |
| Windows x86\_64   | `oxipulse-x86_64-pc-windows-msvc.zip`        |

Each asset has a `.sha256` file next to it on the [Releases page][urls.releases].

### Build from source

```bash
git clone https://github.com/securyblack/oxi-pulse && cd oxi-pulse
cargo build --release
```

Requires Rust 1.85+ (edition 2024).

---

## Configuration

`/etc/oxipulse/config.toml` on Linux, `C:\ProgramData\oxipulse\config.toml` on Windows. Environment variables override the file.

```toml
endpoint = "http://collector.example.com:4317"   # required, OTLP/gRPC
token    = ""                                    # optional, sent as Authorization: Bearer

interval_secs   = 30      # collection and export interval
buffer_max_size = 8640    # exports kept while offline (8640 x 30 s = 72 h)
auto_update     = true    # daily self-update from GitHub Releases
latency_targets = []      # host:port list; default is your endpoint and 1.1.1.1:53
# telemetry_enabled = false  # usage ping to SecuryBlack, see below
```

| Variable | Setting |
|---|---|
| `OXIPULSE_ENDPOINT` | `endpoint` |
| `OXIPULSE_TOKEN` | `token` |
| `OXIPULSE_INTERVAL_SECS` | `interval_secs` |
| `OXIPULSE_BUFFER_MAX` | `buffer_max_size` |
| `OXIPULSE_AUTO_UPDATE` | `auto_update` (`true`/`false`) |
| `OXIPULSE_LATENCY_TARGETS` | `latency_targets` (comma-separated) |
| `OXIPULSE_TELEMETRY` | `telemetry_enabled` (`true`/`false`) |
| `OXIPULSE_HOST_ROOT` | where the host's `/` is mounted when running in a container (Docker image) |
| `RUST_LOG` | log level (`info` by default) |

---

## Metrics

Names follow the [OpenTelemetry semantic conventions for system metrics](https://opentelemetry.io/docs/specs/semconv/system/system-metrics/), the same ones the OTel Collector's hostmetrics receiver uses, so queries and dashboards work for both. Prometheus names are as they appear through the OTel Collector's Prometheus exporter:

| OTLP name | Prometheus name | Unit | Attributes / notes |
|---|---|---|---|
| `system.cpu.utilization` | `system_cpu_utilization_ratio` | 0-1 | All cores together |
| `system.cpu.logical.count` | `system_cpu_logical_count` | CPUs | |
| `system.cpu.load_average.1m` / `5m` / `15m` | `system_cpu_load_average_1m` … | | Not sent on Windows |
| `system.memory.usage` | `system_memory_usage_bytes` | bytes | `state`: `used`, `free` |
| `system.memory.limit` | `system_memory_limit_bytes` | bytes | Total memory |
| `system.paging.usage` | `system_paging_usage_bytes` | bytes | Swap. `state`: `used`, `free`. Not sent without swap |
| `system.filesystem.usage` | `system_filesystem_usage_bytes` | bytes | `device`, `mountpoint`, `type`, `state`: `used`, `free` |
| `system.network.io` | `system_network_io_bytes_total` | bytes | Counter. `device`, `direction`: `receive`, `transmit`. Use `rate()` |
| `system.uptime` | `system_uptime_seconds` | s | |
| `system.network.latency` | `system_network_latency_milliseconds` | ms | OxiPulse only, no convention exists. TCP connect time per `target`, `status`: `success`, `failure` (value -1) |

Resource attributes on every series: `host.name`, `host.os.name`, `host.os.version`, `host.kernel.version`, `host.cpu.model`, `service.name`, `service.version`.

Usage metrics are sent as gauges rather than the spec's UpDownCounter; in Prometheus the series are identical. Versions before 0.4.0 used other names (`system.cpu.usage`, `system.memory.used`, `system.disk.used`, …); see the [changelog](CHANGELOG.md).

---

## What OxiPulse sends, and where

| Destination | When | What |
|---|---|---|
| Your `endpoint` | Every `interval_secs` | The metrics above. Your `token`, if set |
| GitHub Releases | Once a day, unless `auto_update = false` | Latest-release check, and the new binary if there is one |
| Latency targets | Every interval | A TCP connect, to measure latency |
| `api.securyblack.com` | **Only** if the endpoint is SecuryBlack Cloud, or you set `telemetry_enabled = true` | Agent version, OS, arch, interval, uptime, exported-metric count, buffer usage and last error kind. No metric values |

With any other endpoint and default settings, OxiPulse never contacts SecuryBlack and your token only goes to your own endpoint. Details, including privileges, in [SECURITY.md](SECURITY.md).

---

## Known limitations

- The Linux service runs as **root** (the self-updater needs to replace the binary). See [SECURITY.md](SECURITY.md) to run it unprivileged with `auto_update = false`.
- The self-updater downloads over HTTPS but does not verify checksums or signatures yet; the installer does.
- No macOS build, no per-container metrics and no Docker image yet.
- Metric names are not OTel semantic conventions yet (see above).

---

## Service management

### Linux (systemd)

```bash
systemctl status oxipulse
journalctl -fu oxipulse
systemctl restart oxipulse

# Uninstall
systemctl disable --now oxipulse && rm -rf /usr/local/bin/oxipulse /etc/systemd/system/oxipulse.service /etc/oxipulse
```

### Windows (PowerShell)

```powershell
Get-Service OxiPulse
Restart-Service OxiPulse

# Uninstall
Stop-Service OxiPulse; sc.exe delete OxiPulse
```

Logs are written next to the config file (`/etc/oxipulse`, `C:\ProgramData\oxipulse`).

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
# Run against a local OTLP collector (e.g. examples/prometheus-grafana).
# Logs go next to the config (/etc/oxipulse or C:\ProgramData\oxipulse),
# so the process needs write access there.
OXIPULSE_ENDPOINT=http://localhost:4317 OXIPULSE_AUTO_UPDATE=false cargo run

# Build for all platforms (requires WSL2 on Windows for Linux targets)
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-pc-windows-msvc
```

---

## Releases

OxiPulse follows [Semantic Versioning][urls.semver]. Every push to a `v*` tag
triggers the CI/CD pipeline, which cross-compiles binaries for all supported
platforms and publishes them to [GitHub Releases][urls.releases] with SHA256 checksums.

Unless `auto_update = false`, the agent checks for a new release every 24 hours
and applies it, restarting through the service manager.

---

## Other SecuryBlack agents

OxiPulse is maintained by [SecuryBlack](https://securyblack.com), which also publishes these Apache-2.0 agents:

| Agent | Core Focus | Official Website | Repository |
| :--- | :--- | :--- | :--- |
| **OxiPulse** | Server telemetry and OTLP metrics | [oxipulse.dev](https://oxipulse.dev) | [securyblack/oxi-pulse](https://github.com/securyblack/oxi-pulse) |
| **FerroSentry** | Server security audit | [ferrosentry.dev](https://ferrosentry.dev) | [securyblack/ferro-sentry](https://github.com/securyblack/ferro-sentry) |
| **CupraFlow** | Networking/HA development (not operationally verified) | [cupraflow.dev](https://cupraflow.dev) | [securyblack/cupra-flow](https://github.com/securyblack/cupra-flow) |
| **CromoForge** | Docker containers, logs & PostgreSQL management | [cromoforge.dev](https://cromoforge.dev) | [securyblack/cromo-forge](https://github.com/securyblack/cromo-forge) |
| **TitanVault** | Streaming backups (restore workflow not verified) | [titanvault.dev](https://titanvault.dev) | [securyblack/titan-vault](https://github.com/securyblack/titan-vault) |

All of them can report to [SecuryBlack Cloud](https://securyblack.com).

---

## License

OxiPulse is licensed under the [Apache License, Version 2.0][urls.license].

All dependencies are compatible with Apache 2.0 (MIT, Apache-2.0, or BSD licensed).

---

<p align="center">
  Maintained by <strong><a href="https://securyblack.com">SecuryBlack</a></strong>
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

## Maintainer context (workspace)

English is the primary language for this repository's public documentation. Shared product decisions, commercial terms and the backlog live in the [workspace wiki](../sb-wiki/Índice.md), [current state](../sb-wiki/Producto/Estado%20actual.md) and [product log](../sb-wiki/Producto/Bitácora.md). These links require `sb-wiki` as a sibling checkout. Keep agent-specific usage and technical contracts here; update shared decisions in the wiki.
