# Ansible collection `securyblack.oxipulse`

Installs the [OxiPulse](https://github.com/securyblack/oxi-pulse) agent from the release `.deb`/`.rpm`, writes `/etc/oxipulse/config.toml` (mode 0600) and enables the systemd service. Debian, Ubuntu, RHEL, Rocky, Alma and Fedora, on x86_64 and aarch64.

```bash
ansible-galaxy collection install "git+https://github.com/securyblack/oxi-pulse.git#/ansible,main"
```

```yaml
- hosts: all
  become: true
  roles:
    - role: securyblack.oxipulse.oxipulse
      vars:
        oxipulse_endpoint: "http://collector.example.com:4317"
        # oxipulse_token: "{{ vault_oxipulse_token }}"
```

| Variable | Default | |
|---|---|---|
| `oxipulse_endpoint` | | OTLP/gRPC receiver. Required. |
| `oxipulse_token` | `""` | Bearer token, if your backend needs one. Keep it in Ansible Vault. |
| `oxipulse_version` | `0.4.5` | Release to install. Change it and re-run to upgrade. |
| `oxipulse_interval_secs` | `30` | Collection and export interval. |
| `oxipulse_buffer_max_size` | `8640` | Exports kept while the backend is unreachable (72 h at 30 s). |
| `oxipulse_latency_targets` | `[]` | `host:port` list; empty means your endpoint and `1.1.1.1:53`. |
| `oxipulse_telemetry_enabled` | unset | Usage ping to SecuryBlack, off unless the endpoint is SecuryBlack Cloud. See the [main README](../README.md#what-oxipulse-sends-and-where). |
| `oxipulse_state` | `present` | `absent` removes the package. |

The agent's self-update is off: upgrades go through this role.
