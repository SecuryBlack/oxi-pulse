use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, Networks, System};

#[derive(Debug)]
pub struct Metrics {
    pub timestamp_unix_ms: u64,
    pub cpu_usage_percent: f32,
    pub ram_total_bytes: u64,
    pub ram_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub net_bytes_in: u64,
    pub net_bytes_out: u64,
}

pub struct Collector {
    sys: System,
    networks: Networks,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
            networks: Networks::new_with_refreshed_list(),
        }
    }

    pub fn collect(&mut self) -> Metrics {
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

        // Disk — aggregate all mounted disks
        let disks = Disks::new_with_refreshed_list();
        let disk_total_bytes: u64 = disks.iter().map(|d| d.total_space()).sum();
        let disk_used_bytes: u64 = disks
            .iter()
            .map(|d| d.total_space().saturating_sub(d.available_space()))
            .sum();

        // Network — aggregate all interfaces
        self.networks.refresh(true);
        let net_bytes_in: u64 = self.networks.iter().map(|(_, n)| n.received()).sum();
        let net_bytes_out: u64 = self.networks.iter().map(|(_, n)| n.transmitted()).sum();

        Metrics {
            timestamp_unix_ms,
            cpu_usage_percent,
            ram_total_bytes,
            ram_used_bytes,
            disk_total_bytes,
            disk_used_bytes,
            net_bytes_in,
            net_bytes_out,
        }
    }
}
