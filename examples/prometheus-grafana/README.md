# OxiPulse → OTel Collector → Prometheus → Grafana

A minimal self-hosted backend for OxiPulse. No account needed.

```bash
docker compose up -d
```

Then install OxiPulse on each host you want to monitor, pointing it at this machine:

```bash
curl -fsSL https://install.oxipulse.dev | sudo bash -s -- --endpoint http://<this-machine>:4317
```

Open Grafana at `http://<this-machine>:3000` (default login `admin` / `admin`, change it). The home page is the **OxiPulse hosts** dashboard: a table with every host, then CPU, memory, disk, swap, network, load and latency. Pick hosts with the `Host` selector. The dashboard is [grafana/oxipulse-hosts.json](grafana/oxipulse-hosts.json); you can also import it into an existing Grafana that reads these metrics from Prometheus.

To write your own panels, go to **Explore** and try:

| What | PromQL |
|---|---|
| CPU % per host | `system_cpu_usage_percent` |
| Memory used % | `100 * system_memory_used_bytes / system_memory_total_bytes` |
| Disk used % (per mount) | `100 * system_disk_used_bytes / system_disk_total_bytes` |
| Network in / out (bytes/s) | `system_network_receive_bytes_per_second`, `system_network_transmit_bytes_per_second` |
| Load average | `system_cpu_load_average_1m_ratio` |
| Uptime | `system_uptime_seconds` |

Every series carries a `host_name` label, so you can filter or group by host.

Port 4317 is unauthenticated and unencrypted in this example. Keep it on a private network or VPN, or put TLS and auth in front of it before exposing it.
