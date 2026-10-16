//! Linux readings from /proc, /sys/fs/cgroup and systemd. Every reading is
//! skipped when its file is missing (old kernel, container without the host's
//! /proc, other OS), so this compiles and runs everywhere. Rates are computed
//! against the previous collection, so the first one only sets the baseline.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Services reported per ranking (memory and CPU); the union is sent.
/// ponytail: fixed top-N keeps cardinality bounded, make it configurable if someone needs more.
const TOP_SERVICES: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceUsage {
    pub unit: String,
    pub memory_bytes: u64,
    /// Fraction of all CPUs (0-1), like `system.cpu.utilization`.
    pub cpu_utilization: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pressure {
    pub resource: &'static str,
    pub kind: &'static str,
    /// Fraction of the interval (0-1) some/all tasks were stalled on the resource.
    pub value: f64,
}

#[derive(Debug, Default)]
pub struct LinuxStats {
    pub cpu_steal: Option<f64>,
    pub cpu_iowait: Option<f64>,
    pub pressure: Vec<Pressure>,
    /// Processes killed by the OOM killer since the previous collection.
    pub oom_kills: Option<u64>,
    pub failed_units: Option<u64>,
    pub threads: Option<u64>,
    pub services: Vec<ServiceUsage>,
}

/// Cumulative counters from the previous collection.
#[derive(Default)]
pub struct LinuxState {
    at: Option<Instant>,
    cpu: Option<CpuTimes>,
    pressure: HashMap<(&'static str, &'static str), u64>,
    oom_kills: Option<u64>,
    services: HashMap<String, u64>,
    cgroup_root: String,
}

impl LinuxState {
    pub fn new(host_root: Option<&str>) -> Self {
        // In a container the host's cgroup tree is only visible through the host root mount.
        let host =
            host_root.map(|r| format!("{}/sys/fs/cgroup/system.slice", r.trim_end_matches('/')));
        let cgroup_root = host
            .filter(|p| std::path::Path::new(p).is_dir())
            .unwrap_or_else(|| "/sys/fs/cgroup/system.slice".into());
        Self {
            cgroup_root,
            ..Default::default()
        }
    }

    pub async fn read(&mut self, cpu_count: usize) -> LinuxStats {
        let now = Instant::now();
        let elapsed = self.at.map(|t| now.duration_since(t));
        self.at = Some(now);
        let read = |p: &str| std::fs::read_to_string(p).ok();

        let mut stats = LinuxStats {
            threads: read("/proc/loadavg").as_deref().and_then(parse_threads),
            ..Default::default()
        };

        if let Some(cpu) = read("/proc/stat").as_deref().and_then(parse_cpu)
            && let Some(prev) = self.cpu.replace(cpu)
        {
            (stats.cpu_steal, stats.cpu_iowait) = cpu.since(&prev).unzip();
        }

        for resource in ["cpu", "memory", "io"] {
            let Some(text) = read(&format!("/proc/pressure/{resource}")) else {
                continue;
            };
            for (kind, total_us) in parse_pressure(&text) {
                // System-wide "full" CPU pressure is always 0, so it is noise.
                if resource == "cpu" && kind == "full" {
                    continue;
                }
                let prev = self.pressure.insert((resource, kind), total_us);
                if let (Some(prev), Some(dt)) = (prev, elapsed) {
                    let value = total_us.saturating_sub(prev) as f64 / dt.as_micros() as f64;
                    stats.pressure.push(Pressure {
                        resource,
                        kind,
                        value: value.min(1.0),
                    });
                }
            }
        }

        if let Some(kills) = read("/proc/vmstat").as_deref().and_then(parse_oom_kills) {
            stats.oom_kills = self
                .oom_kills
                .replace(kills)
                .map(|p| kills.saturating_sub(p));
        }

        // Before the await, so systemctl's latency does not skew the CPU interval.
        stats.services = self.services(elapsed, cpu_count);
        stats.failed_units = failed_units().await;
        stats
    }

    /// Memory and CPU per systemd service from cgroup v2.
    fn services(&mut self, elapsed: Option<Duration>, cpu_count: usize) -> Vec<ServiceUsage> {
        let mut usage = HashMap::new();
        let mut all = Vec::new();
        for (unit, path) in service_dirs(std::path::Path::new(&self.cgroup_root)) {
            let read = |f: &str| std::fs::read_to_string(path.join(f)).ok();
            let Some(current) = read("memory.current").and_then(|s| s.trim().parse().ok()) else {
                continue;
            };
            let memory_bytes = working_set(current, read("memory.stat").as_deref());
            let cpu_us = read("cpu.stat")
                .as_deref()
                .and_then(|s| stat_value(s, "usage_usec"));
            let mut cpu_utilization = 0.0;
            if let Some(cpu_us) = cpu_us {
                if let (Some(prev), Some(dt)) = (self.services.get(&unit), elapsed) {
                    let busy = cpu_us.saturating_sub(*prev) as f64;
                    cpu_utilization = busy / (dt.as_micros() as f64 * cpu_count.max(1) as f64);
                }
                usage.insert(unit.clone(), cpu_us);
            }
            all.push(ServiceUsage {
                unit,
                memory_bytes,
                cpu_utilization: cpu_utilization.min(1.0),
            });
        }
        // Stopped services drop out instead of piling up.
        self.services = usage;
        top_services(all, TOP_SERVICES)
    }
}

/// `.service` cgroups under `dir`, also inside nested slices such as
/// `system-postgresql.slice`. Does not descend into a service's own subgroups,
/// which its counters already include.
fn service_dirs(dir: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".service") {
            out.push((name, entry.path()));
        } else if name.ends_with(".slice") {
            out.extend(service_dirs(&entry.path()));
        }
    }
    out
}

/// Units in the `failed` state, or None without systemd.
async fn failed_units() -> Option<u64> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let cmd = tokio::process::Command::new("systemctl")
        .args(["list-units", "--state=failed", "--no-legend", "--plain"])
        // A timed-out systemctl is killed instead of piling up.
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(Duration::from_secs(2), cmd)
        .await
        .ok()?
        .ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count() as u64
    })
}

fn top_services(mut all: Vec<ServiceUsage>, n: usize) -> Vec<ServiceUsage> {
    all.sort_by(|a, b| b.cpu_utilization.total_cmp(&a.cpu_utilization));
    let by_cpu: Vec<String> = all
        .iter()
        .take(n)
        .filter(|s| s.cpu_utilization > 0.0)
        .map(|s| s.unit.clone())
        .collect();
    all.sort_by_key(|s| std::cmp::Reverse(s.memory_bytes));
    all.into_iter()
        .enumerate()
        .filter(|(i, s)| *i < n || by_cpu.contains(&s.unit))
        .map(|(_, s)| s)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CpuTimes {
    total: u64,
    iowait: u64,
    steal: u64,
}

impl CpuTimes {
    /// (steal, iowait) as fractions of the time elapsed since `prev`.
    fn since(&self, prev: &CpuTimes) -> Option<(f64, f64)> {
        let dt = self.total.checked_sub(prev.total).filter(|d| *d > 0)? as f64;
        let frac = |now: u64, before: u64| now.saturating_sub(before) as f64 / dt;
        Some((frac(self.steal, prev.steal), frac(self.iowait, prev.iowait)))
    }
}

/// The aggregate `cpu` line of /proc/stat: user nice system idle iowait irq softirq steal ...
fn parse_cpu(text: &str) -> Option<CpuTimes> {
    let line = text.lines().find(|l| l.starts_with("cpu "))?;
    let v: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .map_while(|s| s.parse().ok())
        .collect();
    if v.len() < 8 {
        return None;
    }
    // guest time is already counted in user, so only the first 8 fields add up to the total.
    Some(CpuTimes {
        total: v[..8].iter().sum(),
        iowait: v[4],
        steal: v[7],
    })
}

/// `total=` (µs stalled) of each line of /proc/pressure/<resource>.
fn parse_pressure(text: &str) -> Vec<(&'static str, u64)> {
    text.lines()
        .filter_map(|l| {
            let kind = match l.split_whitespace().next()? {
                "some" => "some",
                "full" => "full",
                _ => return None,
            };
            let total = l
                .split_whitespace()
                .find_map(|f| f.strip_prefix("total="))?;
            Some((kind, total.parse().ok()?))
        })
        .collect()
}

fn parse_oom_kills(text: &str) -> Option<u64> {
    text.lines()
        .find_map(|l| l.strip_prefix("oom_kill "))?
        .trim()
        .parse()
        .ok()
}

/// Fourth field of /proc/loadavg is `running/total` scheduling entities (threads).
fn parse_threads(text: &str) -> Option<u64> {
    text.split_whitespace()
        .nth(3)?
        .split('/')
        .nth(1)?
        .parse()
        .ok()
}

/// One `key value` line of a cgroup flat-keyed file (cpu.stat, memory.stat).
fn stat_value(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(' '))?
        .trim()
        .parse()
        .ok()
}

/// Working set, as docker stats and Kubernetes report it: memory.current
/// minus the page cache the kernel can drop at once. Without this a service
/// that reads many files (FIM hashing, log tailing) shows its cache as RAM.
fn working_set(current: u64, memory_stat: Option<&str>) -> u64 {
    let inactive = memory_stat.and_then(|s| stat_value(s, "inactive_file"));
    current.saturating_sub(inactive.unwrap_or(0))
}

/// (total, used) inodes of the filesystem at `path`, or None where it has no
/// fixed inode table (btrfs, vfat) or on other platforms.
#[cfg(unix)]
pub fn inodes(path: &str) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: valid NUL-terminated path and a zeroed out-struct of the right type.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let total = s.f_files as u64;
    (total > 0).then(|| (total, total.saturating_sub(s.f_ffree as u64)))
}

#[cfg(not(unix))]
pub fn inodes(_path: &str) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_steal_and_iowait_from_proc_stat() {
        let a = parse_cpu("cpu  100 0 50 800 20 0 0 30 0 0\ncpu0 1 2 3").unwrap();
        let b = parse_cpu("cpu  150 0 70 900 40 0 0 40 5 0\n").unwrap();
        assert_eq!(a.total, 1000);
        let (steal, iowait) = b.since(&a).unwrap();
        assert!((steal - 10.0 / 200.0).abs() < 1e-9);
        assert!((iowait - 20.0 / 200.0).abs() < 1e-9);
        assert_eq!(a.since(&a), None);
    }

    #[test]
    fn pressure_totals() {
        let text = "some avg10=0.00 avg60=0.10 avg300=0.00 total=1234\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=56\n";
        assert_eq!(parse_pressure(text), vec![("some", 1234), ("full", 56)]);
    }

    #[test]
    fn small_files() {
        assert_eq!(parse_oom_kills("pgfault 9\noom_kill 3\n"), Some(3));
        assert_eq!(parse_oom_kills("pgfault 9\n"), None);
        assert_eq!(parse_threads("0.10 0.20 0.30 2/345 6789\n"), Some(345));
        assert_eq!(
            stat_value("usage_usec 42\nuser_usec 30\n", "usage_usec"),
            Some(42)
        );
        let mem = "anon 100\nactive_file 50\ninactive_file 800\ninactive_anon 0\n";
        assert_eq!(stat_value(mem, "inactive_file"), Some(800));
        assert_eq!(working_set(1000, Some(mem)), 200);
        assert_eq!(working_set(1000, None), 1000);
        assert_eq!(working_set(500, Some(mem)), 0);
    }

    #[test]
    fn service_dirs_walks_nested_slices() {
        let root = std::env::temp_dir().join(format!("oxipulse-cg-{}", std::process::id()));
        for d in [
            "a.service/sub",
            "system-postgresql.slice/postgresql@16-main.service",
            "init.scope",
        ] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let mut units: Vec<_> = service_dirs(&root).into_iter().map(|(u, _)| u).collect();
        units.sort();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(units, ["a.service", "postgresql@16-main.service"]);
    }

    #[test]
    fn top_services_keeps_memory_and_cpu_leaders() {
        let s = |unit: &str, memory_bytes, cpu_utilization| ServiceUsage {
            unit: unit.into(),
            memory_bytes,
            cpu_utilization,
        };
        let all = vec![
            s("a", 300, 0.0),
            s("b", 200, 0.0),
            s("c", 1, 0.9),
            s("d", 2, 0.0),
        ];
        let units: Vec<_> = top_services(all, 2).into_iter().map(|s| s.unit).collect();
        assert_eq!(units, ["a", "b", "c"]);
    }
}
