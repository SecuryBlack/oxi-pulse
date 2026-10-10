# Benchmark

How much memory and CPU OxiPulse uses on Linux, next to two common ways of collecting the same host metrics. The same run also checks the self-hosted example end to end.

## Results

| Agent | Private memory (RssAnon, avg) | RSS (avg) | RSS (max) | CPU (% of one core) |
|---|---|---|---|---|
| **OxiPulse 0.3.14** | **3.5 MB** | **11.4 MB** | 11.4 MB | **0.01 %** |
| node_exporter 1.12.1 | 10.2 MB | 24.0 MB | 24.3 MB | 0.04 % |
| otelcol-contrib 0.162.0 (hostmetrics) | 41.2 MB | 203.9 MB | 205.2 MB | 0.06 % |

GitHub Actions `ubuntu-24.04` runner: 4 vCPU, 16 GB RAM, Ubuntu 24.04.5, kernel 6.17 (Azure), 2026-10-10. An earlier run the same day gave the same picture (OxiPulse 3.6 MB private, 10.9 MB RSS, 0.01 %).

GitHub Actions run: [run 38045299125](https://github.com/SecuryBlack/oxi-pulse/actions/runs/38045299125)

- **Private memory** (`RssAnon`) is the memory the process really owns. **RSS** also counts shared and file-backed pages, such as the mapped binary, which is why the OTel Collector's RSS is much larger than its private memory.
- **CPU** is the time the process spent on CPU during the measured window, divided by that window: 0.01 % of one core is about 0.03 s of CPU in 5 minutes. The kernel counts in 10 ms ticks, so very small values are approximate.
- Each tool collects a different set of metrics. node_exporter exposes far more series by default; the OTel Collector was configured with the hostmetrics scrapers closest to what OxiPulse collects (cpu, memory, disk, filesystem, network, load, paging). Read the numbers as "the cost of a host metrics agent", not as a feature-for-feature race.

## What the run does

[`run.sh`](run.sh), in order:

1. Starts [`examples/prometheus-grafana`](../examples/prometheus-grafana) with Docker Compose.
2. Installs the latest OxiPulse release with the public installer (`curl -fsSL https://install.oxipulse.dev | sudo bash -s -- --endpoint http://127.0.0.1:4317 --no-auto-update`).
3. Measures OxiPulse, then checks that its metrics are in Prometheus and that the Grafana dashboard is provisioned. The run fails if either is missing.
4. Stops OxiPulse and measures node_exporter (scraped every 30 s), then otelcol-contrib with the hostmetrics receiver (every 30 s, exporting OTLP to the Compose collector).

Every agent runs alone: 60 s of warm-up that is discarded, then 300 s measured. Memory is sampled from `/proc/<pid>/status` every 5 s; CPU comes from `utime + stime` in `/proc/<pid>/stat`.

## Run it yourself

On any Linux machine with systemd, Docker Compose and sudo (it installs the OxiPulse service and uses ports 3000, 4317, 9090 and 9100):

```bash
git clone https://github.com/securyblack/oxi-pulse && cd oxi-pulse
bash bench/run.sh
```

Results are written to `bench/out/results.md`. Or run the **Linux end-to-end and benchmark** workflow from the Actions tab of your fork.
