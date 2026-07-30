---
name: Bug report
about: Create a report to help us improve OxiPulse
title: '[BUG] '
labels: 'bug'
assignees: ''

---

**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Agent configuration used (`config.toml` or env vars with sensitive tokens redacted)
2. Command run / Service state (`systemctl status oxipulse` or `Get-Service OxiPulse`)
3. See error in logs (`journalctl -u oxipulse` or Event Viewer)

**Expected behavior**
A clear and concise description of what you expected to happen.

**Environment (please complete the following information):**
 - OxiPulse Version: [e.g. 0.3.8]
 - OS & Architecture: [e.g. Ubuntu 22.04 LTS x86_64, Windows Server 2022 AMD64]
 - Deployment Mode: [e.g. systemd service, Windows service, local_agent, direct]

**Logs / Screenshots**
If applicable, attach relevant log snippets from `oxipulse`.

**Additional context**
Add any other context about the problem here.

