#!/usr/bin/env bash
# End-to-end check and resource benchmark on Linux.
#
# 1. Starts examples/prometheus-grafana (Collector -> Prometheus -> Grafana).
# 2. Installs OxiPulse with the public installer, exactly as a user would.
# 3. Checks that metrics reach Prometheus and the dashboard is provisioned.
# 4. Measures OxiPulse, node_exporter and the OTel Collector hostmetrics
#    receiver one at a time, same interval (30 s), same machine.
#
# Needs: Linux with systemd, Docker Compose, curl, sudo. Run from the repo root.
# Results go to bench/out/. Takes about 20 minutes.
set -euo pipefail

WARMUP=${WARMUP:-60}       # seconds discarded after start
DURATION=${DURATION:-300}  # seconds measured
NODE_EXPORTER=1.12.1
OTELCOL=0.162.0
OUT=bench/out
mkdir -p "$OUT"
HZ=$(getconf CLK_TCK)

log() { echo "[$(date +%T)] $*"; }

# measure <name> <pid>: CPU over DURATION from /proc/<pid>/stat, memory sampled every 5 s.
measure() {
  local name=$1 pid=$2 t0 t1 rss_sum=0 anon_sum=0 n=0 rss_max=0 rss anon
  sleep "$WARMUP"
  t0=$(awk '{print $14+$15}' "/proc/$pid/stat")
  for _ in $(seq 1 $((DURATION / 5))); do
    rss=$(awk '/^VmRSS/{print $2}' "/proc/$pid/status")
    anon=$(awk '/^RssAnon/{print $2}' "/proc/$pid/status")
    rss_sum=$((rss_sum + rss)); anon_sum=$((anon_sum + anon)); n=$((n + 1))
    [ "$rss" -gt "$rss_max" ] && rss_max=$rss
    sleep 5
  done
  t1=$(awk '{print $14+$15}' "/proc/$pid/stat")
  awk -v name="$name" -v ticks=$((t1 - t0)) -v hz="$HZ" -v dur="$DURATION" \
      -v rss=$((rss_sum / n)) -v anon=$((anon_sum / n)) -v max="$rss_max" \
      'BEGIN { printf "| %s | %.1f MB | %.1f MB | %.1f MB | %.2f %% |\n", name, anon/1024, rss/1024, max/1024, ticks/hz/dur*100 }' \
      >> "$OUT/results.md"
}

cat > "$OUT/results.md" <<EOF
| Agent | Private memory (RssAnon, avg) | RSS (avg) | RSS (max) | CPU (% of one core) |
|---|---|---|---|---|
EOF

log "Starting Collector, Prometheus and Grafana"
docker compose -f examples/prometheus-grafana/docker-compose.yml up -d
until curl -fs localhost:3000/api/health >/dev/null; do sleep 2; done

log "Installing OxiPulse with the public installer"
curl -fsSL https://install.oxipulse.dev | sudo bash -s -- --endpoint http://127.0.0.1:4317 --no-auto-update
oxipulse --version | tee "$OUT/oxipulse-version.txt"

log "Measuring OxiPulse"
measure "OxiPulse $(oxipulse --version | awk '{print $NF}')" "$(systemctl show -p MainPID --value oxipulse)"

log "Checking metrics in Prometheus and the dashboard in Grafana"
for _ in $(seq 1 24); do
  curl -fsG localhost:9090/api/v1/query --data-urlencode 'query=count by (host_name) (system_cpu_usage_percent)' \
    > "$OUT/prometheus.json"
  grep '"host_name"' "$OUT/prometheus.json" >/dev/null && break
  sleep 5
done
grep '"host_name"' "$OUT/prometheus.json" >/dev/null
curl -fs -u admin:admin 'localhost:3000/api/search?query=OxiPulse' > "$OUT/grafana.json"
grep oxipulse-hosts "$OUT/grafana.json" >/dev/null
log "End-to-end check passed"
sudo systemctl stop oxipulse

log "Measuring node_exporter $NODE_EXPORTER (scraped every 30 s)"
curl -fsSL "https://github.com/prometheus/node_exporter/releases/download/v${NODE_EXPORTER}/node_exporter-${NODE_EXPORTER}.linux-amd64.tar.gz" | tar xz -C /tmp
/tmp/node_exporter-${NODE_EXPORTER}.linux-amd64/node_exporter --web.listen-address=127.0.0.1:9100 >/dev/null 2>&1 &
NE=$!
(while sleep 30; do curl -fs 127.0.0.1:9100/metrics >/dev/null || true; done) &
SCRAPER=$!
measure "node_exporter $NODE_EXPORTER" "$NE"
kill "$NE" "$SCRAPER"

log "Measuring otelcol-contrib $OTELCOL with the hostmetrics receiver"
curl -fsSL "https://github.com/open-telemetry/opentelemetry-collector-releases/releases/download/v${OTELCOL}/otelcol-contrib_${OTELCOL}_linux_amd64.tar.gz" | tar xz -C /tmp otelcol-contrib
cat > /tmp/hostmetrics.yaml <<'EOF'
receivers:
  hostmetrics:
    collection_interval: 30s
    scrapers: { cpu: {}, memory: {}, disk: {}, filesystem: {}, network: {}, load: {}, paging: {} }
exporters:
  otlp:
    endpoint: 127.0.0.1:4317
    tls: { insecure: true }
service:
  telemetry: { metrics: { level: none } }
  pipelines: { metrics: { receivers: [hostmetrics], exporters: [otlp] } }
EOF
/tmp/otelcol-contrib --config=/tmp/hostmetrics.yaml >/dev/null 2>&1 &
OC=$!
measure "otelcol-contrib $OTELCOL (hostmetrics)" "$OC"
kill "$OC"

{
  echo
  echo "Machine: $(nproc) vCPU, $(free -m | awk '/^Mem/{print $2}') MB RAM, $(. /etc/os-release; echo "$PRETTY_NAME"), kernel $(uname -r)."
  echo "Each agent ran alone for ${WARMUP} s of warm-up plus ${DURATION} s measured, collecting every 30 s."
} >> "$OUT/results.md"
cat "$OUT/results.md"
