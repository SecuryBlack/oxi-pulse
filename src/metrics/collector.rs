use sb_agent_core::net::parse_host_port;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, Networks, System};
use tokio::net::TcpStream;

#[derive(Debug, Clone)]
pub struct LatencyMetric {
    pub target: String,
    pub latency_ms: Option<f64>,
}

#[derive(Debug)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

/// Bytes moved by one network interface since the previous collection.
#[derive(Debug)]
pub struct NetIo {
    pub device: String,
    pub received: u64,
    pub transmitted: u64,
}

#[derive(Debug)]
pub struct Metrics {
    #[allow(dead_code)]
    pub timestamp_unix_ms: u64,
    pub uptime_secs: u64,
    pub cpu_count: usize,
    pub cpu_usage_percent: f32,
    pub load_avg_1m: f64,
    pub load_avg_5m: f64,
    pub load_avg_15m: f64,
    pub ram_total_bytes: u64,
    pub ram_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetIo>,
    /// Network latency metrics to various targets in milliseconds.
    pub latencies: Vec<LatencyMetric>,
}

pub struct Collector {
    sys: System,
    networks: Networks,
    /// Where the host's `/` is mounted when running in a container (`OXIPULSE_HOST_ROOT`).
    host_root: Option<String>,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
            networks: Networks::new_with_refreshed_list(),
            host_root: std::env::var("OXIPULSE_HOST_ROOT")
                .ok()
                .filter(|v| !v.is_empty()),
        }
    }

    pub async fn collect(&mut self, latency_targets: &[String], default_endpoint: &str) -> Metrics {
        // Timestamp at moment of reading
        let timestamp_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // System Uptime & CPU Cores & Load Average
        let uptime_secs = System::uptime();
        let cpu_count = self.sys.cpus().len();
        let load_avg = System::load_average();

        // CPU — requires two refreshes with a short gap for an accurate reading;
        // on the first call sysinfo returns 0, subsequent calls return real usage.
        self.sys.refresh_cpu_usage();
        let cpu_usage_percent = self.sys.global_cpu_usage();

        // RAM & Swap
        self.sys.refresh_memory();
        let ram_total_bytes = self.sys.total_memory();
        let ram_used_bytes = self.sys.used_memory();
        let swap_total_bytes = self.sys.total_swap();
        let swap_used_bytes = self.sys.used_swap();

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
                    mount_point: d.mount_point().to_string_lossy().to_string(),
                    fs_type: d.file_system().to_string_lossy().to_string(),
                    total_bytes: d.total_space(),
                    used_bytes: d.total_space().saturating_sub(d.available_space()),
                }
            })
            .collect();
        let disk_infos = keep_disks(disk_infos, self.host_root.as_deref());

        // Network — bytes per interface since the previous refresh
        self.networks.refresh(true);
        let networks = self
            .networks
            .iter()
            .map(|(device, n)| NetIo {
                device: device.clone(),
                received: n.received(),
                transmitted: n.transmitted(),
            })
            .collect();

        // Measure latencies concurrently
        let mut ping_tasks = Vec::new();
        let targets_to_ping = if latency_targets.is_empty() {
            vec![default_endpoint.to_string(), "1.1.1.1:53".to_string()]
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
            uptime_secs,
            cpu_count,
            cpu_usage_percent,
            load_avg_1m: load_avg.one,
            load_avg_5m: load_avg.five,
            load_avg_15m: load_avg.fifteen,
            ram_total_bytes,
            ram_used_bytes,
            swap_total_bytes,
            swap_used_bytes,
            disks: disk_infos,
            networks,
            latencies,
        }
    }
}

/// Drops container overlays, pseudo filesystems (size 0) and repeated mounts of the same device (bind mounts),
/// so each filesystem is reported once. With `host_root` (the host's `/` mounted
/// in a container), keeps only the host's mounts, with the prefix removed.
fn keep_disks(disks: Vec<DiskInfo>, host_root: Option<&str>) -> Vec<DiskInfo> {
    let mut seen = std::collections::HashSet::new();
    disks
        .into_iter()
        .filter(|d| d.fs_type != "overlay" && d.total_bytes > 0)
        .filter_map(|mut d| {
            if let Some(root) = host_root {
                let rest = d.mount_point.strip_prefix(root.trim_end_matches('/'))?;
                if !rest.is_empty() && !rest.starts_with('/') {
                    return None;
                }
                // What sysinfo skips on the host, now visible under the prefix.
                if ["/sys", "/proc", "/dev", "/run"]
                    .iter()
                    .any(|p| rest == *p || rest.starts_with(&format!("{p}/")))
                    && !rest.starts_with("/run/media")
                {
                    return None;
                }
                d.mount_point = if rest.is_empty() {
                    "/".into()
                } else {
                    rest.into()
                };
            }
            seen.insert(d.name.clone()).then_some(d)
        })
        .collect()
}

/// OS name and version from `<host_root>/etc/os-release`, so a container
/// reports the host's distribution instead of the image's.
pub fn host_os_release(host_root: &str) -> Option<(String, Option<String>)> {
    let path = format!("{}/etc/os-release", host_root.trim_end_matches('/'));
    let text = std::fs::read_to_string(path).ok()?;
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').to_string())
    };
    Some((field("NAME")?, field("VERSION_ID")))
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
        if let Ok(Ok(_)) =
            tokio::time::timeout(std::time::Duration::from_secs(2), TcpStream::connect(sa)).await
        {
            return Some(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(name: &str, mount: &str, fs: &str) -> DiskInfo {
        DiskInfo {
            name: name.into(),
            mount_point: mount.into(),
            fs_type: fs.into(),
            total_bytes: 100,
            used_bytes: 10,
        }
    }

    #[test]
    fn keep_disks_in_container() {
        let raw = vec![
            disk("overlay", "/", "overlay"),
            disk("/dev/sda1", "/etc/hosts", "ext4"),
            disk("/dev/sda1", "/hostfs", "ext4"),
            disk("/dev/sda15", "/hostfs/boot/efi", "vfat"),
            disk("/dev/sdb1", "/hostfsdata", "ext4"),
            disk(
                "overlay",
                "/hostfs/var/lib/docker/overlay2/x/merged",
                "overlay",
            ),
            disk("efivarfs", "/hostfs/sys/firmware/efi/efivars", "efivarfs"),
            DiskInfo {
                total_bytes: 0,
                ..disk("bpf", "/hostfs/sys/fs/bpf", "bpf")
            },
        ];
        let kept: Vec<_> = keep_disks(raw, Some("/hostfs"))
            .into_iter()
            .map(|d| (d.name, d.mount_point))
            .collect();
        assert_eq!(
            kept,
            vec![
                ("/dev/sda1".to_string(), "/".to_string()),
                ("/dev/sda15".to_string(), "/boot/efi".to_string()),
            ]
        );
    }

    #[test]
    fn keep_disks_on_host_drops_overlays_and_bind_mounts() {
        let raw = vec![
            disk("/dev/sda1", "/", "ext4"),
            disk("/dev/sda1", "/var/snap/foo", "ext4"),
            disk("overlay", "/var/lib/docker/overlay2/x/merged", "overlay"),
        ];
        assert_eq!(keep_disks(raw, None).len(), 1);
    }

    #[tokio::test]
    async fn test_collector_creation_and_collection() {
        let mut collector = Collector::new();
        let metrics = collector.collect(&[], "127.0.0.1:4317").await;

        assert!(metrics.ram_total_bytes > 0);
        assert!(metrics.timestamp_unix_ms > 0);
        assert!(!metrics.latencies.is_empty());
    }
}
