# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.1.13...HEAD
[0.1.13]: https://github.com/SecuryBlack/oxi-pulse/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/SecuryBlack/oxi-pulse/releases/tag/v0.1.12
[0.1.11]: https://github.com/SecuryBlack/oxi-pulse/releases/tag/v0.1.11
[0.1.10]: https://github.com/SecuryBlack/oxi-pulse/releases/tag/v0.1.10
