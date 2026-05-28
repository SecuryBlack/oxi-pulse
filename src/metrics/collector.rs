use std::time::{Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, Networks, System};
use tokio::net::TcpStream;
use crate::buffer::parse_host_port;

#[derive(Debug, Clone)]
pub struct LatencyMetric {
    pub target: String,
    pub latency_ms: Option<f64>,
}

#[derive(Debug)]
pub struct DiskInfo {
    pub name: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug)]
pub struct Metrics {
    #[allow(dead_code)]
    pub timestamp_unix_ms: u64,
    pub cpu_usage_percent: f32,
    pub ram_total_bytes: u64,
    pub ram_used_bytes: u64,
    pub disks: Vec<DiskInfo>,
    /// Network receive throughput in bytes per second.
    pub net_bps_in: f64,
    /// Network transmit throughput in bytes per second.
    pub net_bps_out: f64,
    /// Network latency metrics to various targets in milliseconds.
    pub latencies: Vec<LatencyMetric>,
}

pub struct Collector {
    sys: System,
    networks: Networks,
    last_collect: Option<Instant>,
    last_net_bytes_in: u64,
    last_net_bytes_out: u64,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
            networks: Networks::new_with_refreshed_list(),
            last_collect: None,
            last_net_bytes_in: 0,
            last_net_bytes_out: 0,
        }
    }

    pub async fn collect(&mut self, latency_targets: &[String], default_endpoint: &str) -> Metrics {
        // Timestamp at moment of reading
        let timestamp_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // CPU — requires two refreshes with a short gap for an accurate reading;
        // on the first call sysinfo returns 0, subsequent calls return real usage.
        self.sys.refresh_cpu_usage();
        let cpu_usage_percent = self.sys.global_cpu_usage();

        // RAM
        self.sys.refresh_memory();
        let ram_total_bytes = self.sys.total_memory();
        let ram_used_bytes = self.sys.used_memory();

        // Disk — collect per-disk info
        let disks = Disks::new_with_refreshed_list();
        let disk_infos: Vec<DiskInfo> = disks
            .iter()
            .map(|d| {
                let name = if cfg!(target_os = "windows") {
                    let mut n = d.mount_point().to_string_lossy().to_string();
                    n = n.trim_end_matches(&['\\', '/'][..]).to_string();
                    if n.is_empty() {
                        d.name().to_string_lossy().to_string()
                    } else {
                        n
                    }
                } else {
                    let n = d.name().to_string_lossy().to_string();
                    if n.is_empty() {
                        d.mount_point().to_string_lossy().to_string()
                    } else {
                        n
                    }
                };
                DiskInfo {
                    name,
                    total_bytes: d.total_space(),
                    used_bytes: d.total_space().saturating_sub(d.available_space()),
                }
            })
            .collect();

        // Network — aggregate all interfaces and compute throughput
        self.networks.refresh(true);
        let raw_bytes_in: u64 = self.networks.iter().map(|(_, n)| n.received()).sum();
        let raw_bytes_out: u64 = self.networks.iter().map(|(_, n)| n.transmitted()).sum();

        let now = Instant::now();
        let (net_bps_in, net_bps_out) = match self.last_collect {
            Some(last) => {
                let elapsed_secs = last.elapsed().as_secs_f64();
                if elapsed_secs > 0.0 {
                    let delta_in = raw_bytes_in.saturating_sub(self.last_net_bytes_in);
                    let delta_out = raw_bytes_out.saturating_sub(self.last_net_bytes_out);
                    (
                        delta_in as f64 / elapsed_secs,
                        delta_out as f64 / elapsed_secs,
                    )
                } else {
                    (0.0, 0.0)
                }
            }
            None => {
                // First collection: no previous data to compare against.
                (0.0, 0.0)
            }
        };

        self.last_collect = Some(now);
        self.last_net_bytes_in = raw_bytes_in;
        self.last_net_bytes_out = raw_bytes_out;

        // Measure latencies concurrently
        let mut ping_tasks = Vec::new();
        let targets_to_ping = if latency_targets.is_empty() {
            vec![default_endpoint.to_string()]
        } else {
            latency_targets.to_vec()
        };

        for target in targets_to_ping {
            let task = tokio::spawn(async move {
                let ms = ping_target(&target).await;
                LatencyMetric {
                    target,
                    latency_ms: ms,
                }
            });
            ping_tasks.push(task);
        }

        let mut latencies = Vec::new();
        for task in ping_tasks {
            if let Ok(res) = task.await {
                latencies.push(res);
            }
        }

        Metrics {
            timestamp_unix_ms,
            cpu_usage_percent,
            ram_total_bytes,
            ram_used_bytes,
            disks: disk_infos,
            net_bps_in,
            net_bps_out,
            latencies,
        }
    }
}

/// Helper function to perform a single TCP ping.
async fn ping_target(target: &str) -> Option<f64> {
    let addr = parse_host_port(target)?;

    let mut addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host(&addr).await.ok()?.collect();
    if addrs.is_empty() {
        return None;
    }
    // Prefer IPv4 to avoid slow timeouts on systems without functional IPv6 config
    addrs.sort_by_key(|a| if a.is_ipv4() { 0u8 } else { 1u8 });

    for sa in addrs {
        let start = Instant::now();
        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            TcpStream::connect(sa),
        )
        .await
        {
            Ok(Ok(_)) => {
                return Some(start.elapsed().as_secs_f64() * 1000.0);
            }
            _ => {}
        }
    }
    None
}
