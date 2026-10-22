# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Metrics (Linux)**: Failed systemd units no longer count the ones a timer starts (`fwupd-refresh`, `apt-daily`…). They fail when the network hiccups and retry on their own, so a fresh Ubuntu server showed "2 failed services" with nothing wrong.

## [0.5.1] - 2026-10-16

### Fixed
- **Metrics (Linux)**: Per-service memory is now the working set (`memory.current` minus `inactive_file`), as `docker stats` and Kubernetes report it. It used to include reclaimable page cache, so a service that reads many files showed its cache as RAM.

## [0.5.0] - 2026-10-16

### Added
- **Metrics (Linux)**: CPU steal and iowait, pressure stall (PSI) for cpu/memory/io, OOM kills, failed systemd units, thread count, and memory and CPU of the top systemd services from cgroup v2. Inode usage per filesystem on every Unix.

### Changed
- **Latency**: No pings by default. `latency_targets` used to fall back to the endpoint and `1.1.1.1:53`; a successful export already proves the endpoint is reachable.

## [0.4.6] - 2026-10-16

### Security
- **Commands**: Verify S4 signed commands (ed25519 signature, expiry, replay protection) via `sb-agent-core` `command_intake`. During the transition any verification failure (unsigned, invalid or expired) is accepted with a warning.

## [0.4.5] - 2026-10-14

### Security
- **Auto-update**: Only installs ed25519-signed releases (sb-agent-core 0.3). Releases are signed offline before they are published; the release workflow no longer publishes them.

## [0.4.4] - 2026-10-12

### Security
- **Dependencies**: rustls 0.23.45 and lru 0.18 (through ratatui 0.30 in sb-agent-core 0.2), clearing the open RustSec/GitHub advisories.

### Fixed
- **Config**: a config file with non-UTF-8 bytes is read without losing data, and a logging init failure no longer panics (sb-agent-core 0.2).

## [0.4.3] - 2026-10-12

### Added
- **Config**: `headers` (`OXIPULSE_HEADERS`) for backends that want their own gRPC headers, such as OpenObserve (Basic auth, organization), Uptrace (`uptrace-dsn`) or SigNoz Cloud (`signoz-ingestion-key`). `token` is still sent as a Bearer header unless `headers` sets `authorization`.
- **Docs**: [backend guides](docs/backends/) for SigNoz, OpenObserve, Uptrace and VictoriaMetrics; all but SigNoz are tested end to end in CI (#15).
- **Logs**: every failed export is logged at debug level with its gRPC status (`RUST_LOG=oxipulse=debug`).

## [0.4.2] - 2026-10-10

### Added
- **Packages**: `.deb` and `.rpm` for amd64 and arm64 in every release. They install a systemd service that starts once `endpoint` is set; self-update is off in packaged installs.

## [0.4.1] - 2026-10-10

### Added
- **Docker**: Multi-arch image `ghcr.io/securyblack/oxipulse` (amd64, arm64), distroless, non-root, about 33 MB. Run it with `--network host`, `-v /:/hostfs:ro` and `OXIPULSE_HOST_ROOT=/hostfs`; it then reports the host's disks, OS and hostname.
- **Config**: `OXIPULSE_HOST_ROOT` for running in a container.

### Fixed
- **Linux compatibility**: Linux binaries need glibc 2.17 or later (built with cargo-zigbuild). They previously needed glibc 2.39, so they did not start on Ubuntu 22.04, Debian 12, RHEL 8/9 and older.
- **Disks**: Container overlays, pseudo filesystems and repeated mounts of the same device (bind mounts) are no longer reported as separate disks.
- **Offline buffer**: Metrics collected while the backend was unreachable were flushed as a burst of gauge updates, so only the last value of each metric survived. The exporter now keeps every export the backend did not accept, with its original timestamps, and sends them in order once it is reachable. Up to `buffer_max_size` exports (72 h at the default interval), about 2-3 KB each.

## [0.4.0] - 2026-10-10

### Changed
- **Breaking — metric names**: Metrics follow the OpenTelemetry semantic conventions for system metrics, like the OTel Collector hostmetrics receiver (#13). Update queries and dashboards:

  | Before | Now |
  |---|---|
  | `system.cpu.usage` (%) | `system.cpu.utilization` (0-1) |
  | `system.cpu.count` | `system.cpu.logical.count` |
  | `system.memory.used` / `total` | `system.memory.usage{state=used}` / `system.memory.limit` |
  | `system.memory.swap.used` / `total` | `system.paging.usage{state=used\|free}` |
  | `system.disk.used` / `total` (`disk.name`) | `system.filesystem.usage{device, mountpoint, type, state=used\|free}` |
  | `system.network.receive` / `transmit` (bytes/s gauges) | `system.network.io{device, direction}` (byte counter per interface; use `rate()`) |

  `system.uptime`, `system.cpu.load_average.*` and `system.network.latency` keep their names; load average is no longer sent on Windows, where it was always 0. Network bytes recorded while offline are no longer lost, because the counter adds them on flush. SecuryBlack Cloud accepts both naming schemes.

### Added
- **Examples**: Grafana dashboard "OxiPulse hosts" (`examples/prometheus-grafana/grafana/oxipulse-hosts.json`), provisioned as the home page of the Docker Compose example.
- **Benchmark**: `bench/run.sh` and a manual GitHub Actions workflow that check the self-hosted example end to end on Linux and measure OxiPulse, node_exporter and the OTel Collector hostmetrics receiver.

## [0.3.14] - 2026-10-07

### Changed
- **Privacy**: Usage telemetry to SecuryBlack is now off by default. With `telemetry_enabled` unset, the agent only fetches the SecuryBlack remote config when the endpoint is SecuryBlack Cloud. Previously it sent the configured token to `api.securyblack.com` regardless of the endpoint.
- **Config**: `token` is optional; no `Authorization` header is sent when it is empty, for collectors without auth.
- **Installer**: The token prompt accepts an empty value and is skipped when there is no terminal.

### Added
- **Config**: `auto_update` (`OXIPULSE_AUTO_UPDATE`) to disable the daily self-update. Installer flags `--no-auto-update` / `-NoAutoUpdate`.
- **Telemetry**: `host.name` resource attribute, so several hosts can share one backend.
- **Docs**: Self-hosted example (`examples/prometheus-grafana`), list of outbound connections, known limitations, and corrected metric names and privilege notes in README and SECURITY.md.

## [0.3.13] - 2026-09-15

### Added
- **Telemetry**: Report host CPU model, core counts, architecture, and detailed OS distribution in resource attributes to OTLP gateway.
- **Installer**: Support execution policy bypass and forced TLS 1.2+ for Windows Server 2019 compatibility.

## [0.3.12] - 2026-09-15

### Added
- **Telemetry**: Collect and export system load averages (1m, 5m, 15m), swap space metrics, and detailed host uptime.
- **Networking**: Normalize endpoint scheme parsing and enforce TLS strictly on HTTPS endpoints.

## [0.3.11] - 2026-08-24

### Added
- **Commands**: Implement `update_now` management command allowing on-demand manual agent updates triggered remotely from the dashboard.

## [0.3.10] - 2026-08-23

### Changed
- **Dependencies**: Consume `sb-agent-core` from crates.io registry.

## [0.3.9] - 2026-08-23

### Changed
- **Architecture**: Complete retrofit onto shared `sb-agent-core` runtime (unified config parser, updater, structured logging, service lifecycle, status socket, and interactive TUI `oxipulse top` / `oxipulse status`).

## [0.3.8] - 2026-07-28

### Added
- **Config**: Auto-write current version into `config.toml` upon agent startup if missing or outdated.

## [0.3.7] - 2026-07-25

### Fixed
- **Installer**: Improve PowerShell 5.1 compatibility by ensuring TLS 1.2 usage and refactoring syntax.

## [0.3.5] - 2026-07-20

### Changed
- **Updater**: Reduce initial startup update check delay from 5 minutes to 1 minute.

## [0.3.3] - 2026-07-15

### Fixed
- **Buffer**: Clean authority in `parse_host_port` to properly handle OTLP URL endpoints containing paths.

## [0.3.2] - 2026-07-10

### Added
- **Metrics**: Add Cloudflare DNS (`1.1.1.1:53`) as a default fallback latency target when no targets are specified.

## [0.3.1] - 2026-07-05

### Added
- **Metrics**: Implement dynamic network latency metrics via concurrent TCP pings.

## [0.3.0] - 2026-06-28

### Added
- **Metrics**: Send per-disk metrics using the `disk.name` attribute (using mount point or short device name).

### Fixed
- **Metrics**: Normalize disk names and mount points consistently across Linux and Windows.

## [0.1.14] - 2026-06-10

### Added
- **CLI**: Add `--version` / `-V` flag for agent version discovery.
- **Telemetry**: Include `agent_type` field in usage pings.

### Fixed
- **Metrics**: Report real-time network throughput (bytes/sec delta) instead of cumulative totals.

## [0.1.13] - 2026-05-07

### Fixed
- **Windows**: Eliminate VCRUNTIME140.dll dependency by statically linking the MSVC C runtime. The binary is now fully self-contained and runs on clean Windows installations without requiring Visual C++ Redistributable.
- **Windows Installer**: Stop service before replacing binary during updates to prevent file-in-use errors.

### Added
- **Local Agent Mode**: Support `mode = "local_agent"` in config for integration with nexus-agent. When enabled, metrics are sent to `localhost:4317` instead of directly to the cloud endpoint.

## [0.1.10] - 2026-04-15

### Added
- Initial stable release with Windows service support.
- OpenTelemetry OTLP metrics export (CPU, RAM, disk, network).
- Self-updating mechanism via GitHub Releases.
- Offline buffer with exponential backoff for network outages.

[Unreleased]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.4.5...HEAD
[0.4.5]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.4.4...v0.4.5
[0.4.4]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.14...v0.4.0
[0.3.14]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.13...v0.3.14
[0.3.13]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.12...v0.3.13
[0.3.12]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.11...v0.3.12
[0.3.11]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.10...v0.3.11
[0.3.10]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.9...v0.3.10
[0.3.9]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.8...v0.3.9
[0.3.8]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.7...v0.3.8
[0.3.7]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.6...v0.3.7
[0.3.5]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.4...v0.3.5
[0.3.3]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.1.14...v0.3.0
[0.1.14]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.1.13...v0.1.14
[0.1.13]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/SecuryBlack/oxi-pulse/releases/tag/v0.1.12
[0.1.11]: https://github.com/SecuryBlack/oxi-pulse/releases/tag/v0.1.11
[0.1.10]: https://github.com/SecuryBlack/oxi-pulse/releases/tag/v0.1.10

