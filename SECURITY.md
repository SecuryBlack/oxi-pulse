# Security Policy

## Supported Versions

SecuryBlack actively supports and maintains the latest minor release line of OxiPulse with security updates and bug fixes.

| Version | Supported          | Status |
| ------- | ------------------ | ------ |
| 0.3.x   | :white_check_mark: | Active support |
| < 0.3.0 | :x:                | End of life |

---

## Privileges, Sudo & Superuser Safety

Engineers and DevOps teams rightfully inspect open source code before running installation scripts with `sudo` or granting elevated privileges.

### 1. Installation Privileges
The install script requests `sudo` solely to:
- Place the static binary into `/usr/local/bin/oxipulse` (owned by root, mode `0755`).
- Create an unprivileged system user and group: `oxipulse:oxipulse`.
- Register and start the systemd unit file at `/etc/systemd/system/oxipulse.service`.

### 2. Runtime Isolation (Zero Superuser Privileges)
Once installed and running as a systemd service:
- **No root execution**: The daemon drops root and runs as the isolated, unprivileged `oxipulse` system user.
- **Linux Capabilities**: It only retains minimal capabilities (`CAP_NET_RAW` for network latency checks) where explicitly configured.
- **No Remote Command Execution**: OxiPulse is strictly a read-only observability agent. It contains **zero** remote command execution, shell spawning (`/bin/sh`), or dynamic code evaluation capabilities.
- **Read-Only System Access**: All telemetry metrics are gathered by reading kernel pseudo-filesystems (`/proc` and `/sys`). The agent does not modify system configuration or network rules.

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
