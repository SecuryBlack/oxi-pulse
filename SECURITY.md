# Security Policy

## Supported Versions

SecuryBlack actively supports and maintains the latest minor release line of OxiPulse with security updates and bug fixes.

| Version | Supported          | Status |
| ------- | ------------------ | ------ |
| 0.3.x   | :white_check_mark: | Active support |
| < 0.3.0 | :x:                | End of life |

---

## Privileges and network access

You should know exactly what an agent does before running its installer with `sudo`. This section describes the current behaviour of OxiPulse, including its limitations.

### Installation (Linux)

The install script needs root to:

- place the binary at `/usr/local/bin/oxipulse`;
- write `/etc/oxipulse/config.toml` (directory mode `0700`, file mode `0600`, because it may contain your collector token);
- register and start the systemd unit `/etc/systemd/system/oxipulse.service`.

It downloads the release archive from GitHub Releases and verifies it against the published SHA-256 checksum before installing it.

### Runtime

- **The service currently runs as root.** The generated unit has no `User=` directive, because the self-updater needs to replace the binary in `/usr/local/bin`. Running as a dedicated unprivileged user is on the roadmap. If you need it today, disable self-update (`auto_update = false`) and add `User=`/`Group=` plus your own hardening to the unit with `systemctl edit oxipulse`.
- **Metrics are read-only.** CPU, memory, swap, disk, network, load and uptime come from the operating system (`/proc` and `/sys` on Linux, via the `sysinfo` crate). The agent does not change system configuration.
- **No shell or remote code execution.** There is no shell spawning or dynamic evaluation. The only command the agent accepts is `update_now`, through a local Unix socket / named pipe used by the SecuryBlack companion agent; it triggers the same GitHub Releases update check as the daily updater.

### Outbound connections

| Destination | When | What is sent |
|---|---|---|
| Your OTLP `endpoint` | Every `interval_secs` (default 30 s) | The metrics listed in the README and host attributes (host name, OS, kernel, CPU model). Your `token`, if set, as `Authorization: Bearer` |
| `api.github.com` / GitHub Releases | Once a day, unless `auto_update = false` | A request for the latest release; if there is a newer one, downloads it over HTTPS and replaces the binary. The updater does not verify checksums or signatures yet (the installer does). Set `auto_update = false` if your policy requires reviewed upgrades |
| Latency targets | Every interval | A TCP connection to your endpoint and to `1.1.1.1:53` (default) to measure latency. Override with `latency_targets` |
| `api.securyblack.com` | **Only** when the endpoint is SecuryBlack Cloud, or when you set `telemetry_enabled = true` | Agent version, OS, architecture, interval, uptime, exported-metric count, buffer usage, last error kind. No metric values |

With any non-SecuryBlack endpoint and default settings, OxiPulse never contacts SecuryBlack and never sends your token anywhere except your own endpoint.

---

## Reporting a Vulnerability

We take the security of OxiPulse and our users' infrastructure seriously. If you discover a security vulnerability in OxiPulse, please report it through private channels.

**Please DO NOT open public GitHub issues, discussions, or pull requests for security vulnerabilities.**

Instead, please send a private email to:
**security@securyblack.com**

Please include in your report:
- A clear description of the vulnerability and potential security impact.
- Step-by-step reproduction instructions or a minimal proof-of-concept (PoC).
- Affected environment, architecture (`x86_64` or `aarch64`), and OxiPulse version.
- Any suggested mitigations or patches, if available.

### What to Expect

- **Acknowledgment**: We will acknowledge receipt of your vulnerability report within **48 hours**.
- **Assessment**: Our core security team will investigate, reproduce, and confirm the report within **5 business days**.
- **Remediation & Advisory**: Once confirmed, we will develop a patch, release an updated version, and coordinate responsible public disclosure, crediting your contribution unless you prefer to remain anonymous.
