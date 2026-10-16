#!/usr/bin/env bash
# End-to-end check of one backend guide (docs/backends/<backend>.md):
# start the backend as the guide says, run ./oxipulse against it with the
# guide's endpoint and headers, and wait until a metric can be queried.
# Usage: backend-test.sh <openobserve|uptrace|victoriametrics>
set -euo pipefail
backend=$1
log() { echo "[$(date +%T)] $*"; }

# run_agent <endpoint> [headers]: OxiPulse with a 5 s interval, no update, no telemetry
run_agent() {
  OXIPULSE_ENDPOINT=$1 OXIPULSE_HEADERS=${2:-} OXIPULSE_INTERVAL_SECS=5 OXIPULSE_LATENCY_TARGETS=${LATENCY_TARGETS:-} \
  OXIPULSE_AUTO_UPDATE=false OXIPULSE_TELEMETRY=false RUST_LOG=info,oxipulse=debug \
    nohup ./oxipulse > /tmp/oxipulse.log 2>&1 &
}

# wait_for <description> <command>: retry for 3 minutes
wait_for() {
  for _ in $(seq 1 36); do
    if out=$(eval "$2" 2>/tmp/check.err) && [ -n "$out" ] && [ "$out" != 0 ]; then
      log "OK: $1 -> $out"; return 0
    fi
    sleep 5
  done
  log "FAILED: $1 (last output: ${out:-none})"; cat /tmp/check.err; return 1
}

case $backend in
openobserve)
  docker run -d --name openobserve -p 5080:5080 -p 5081:5081 \
    -e ZO_ROOT_USER_EMAIL=root@example.com -e ZO_ROOT_USER_PASSWORD='Complexpass#123' \
    -e ZO_DATA_DIR=/data public.ecr.aws/zinclabs/openobserve:latest
  auth=$(printf '%s' 'root@example.com:Complexpass#123' | base64)
  run_agent http://localhost:5081 "authorization=Basic $auth,organization=default"
  wait_for "system_cpu_utilization stream" \
    "curl -fs -u 'root@example.com:Complexpass#123' 'http://localhost:5080/api/default/streams?type=metrics&fetchSchema=false' | grep -o '\"system_cpu_utilization\"'"
  ;;

uptrace)
  git clone --depth 1 https://github.com/uptrace/uptrace /tmp/uptrace
  (cd /tmp/uptrace/example/docker && docker compose up -d)
  until curl -fs localhost:14318 >/dev/null; do sleep 3; done
  # Latency is off by default since 0.5.0; it is the one metric only OxiPulse sends here.
  LATENCY_TARGETS=localhost:14317 run_agent http://localhost:14317 "uptrace-dsn=http://project1_secret@localhost:14318?grpc=14317"
  ch() { curl -fs "http://uptrace:uptrace@localhost:8123/?database=uptrace" --data-binary "$1"; }
  # ponytail: searches every table with a `metric` column, since Uptrace v2 renamed its schema
  wait_for "system_network_latency (OxiPulse only) in Uptrace" '
    for t in $(ch "SELECT table FROM system.columns WHERE database = '"'"'uptrace'"'"' AND name = '"'"'metric'"'"'"); do
      ch "SELECT count() FROM $t WHERE metric LIKE '"'"'system_network_latency%'"'"'"
    done | awk "{s+=\$1} END {print s+0}"'
  ;;

victoriametrics)
  docker compose -f examples/victoriametrics/docker-compose.yml up -d
  run_agent http://localhost:4317
  wait_for "system_cpu_utilization_ratio with host_name" \
    "curl -fs 'localhost:8428/api/v1/query?query=system_cpu_utilization_ratio' | grep -o '\"host_name\":\"[^\"]*\"'"
  ;;

*) echo "unknown backend: $backend"; exit 2 ;;
esac
