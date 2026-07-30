# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.3.8...HEAD
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

