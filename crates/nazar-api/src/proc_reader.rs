//! /proc-based system metric readers for Linux.
//!
//! `ProcReader` holds state needed for delta-based metrics (CPU usage requires
//! comparing two consecutive reads of /proc/stat).

use nazar_core::*;
use std::collections::HashMap;

/// Accumulated CPU time counters from a single `cpu` line in /proc/stat.
#[derive(Debug, Clone, Default)]
struct CpuTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl CpuTimes {
    fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + self.irq + self.softirq + self.steal
    }

    fn busy(&self) -> u64 {
        self.total() - self.idle - self.iowait
    }
}

/// Reads system metrics from /proc on Linux.
///
/// Holds previous CPU readings so it can compute delta-based usage percentages.
pub struct ProcReader {
    prev_cpu_times: Option<Vec<CpuTimes>>,
}

impl ProcReader {
    pub fn new() -> Self {
        Self {
            prev_cpu_times: None,
        }
    }

    /// Read CPU metrics from /proc/stat and /proc/loadavg.
    ///
    /// The first call returns 0% usage (no previous sample to diff against).
    /// Subsequent calls return real usage based on the delta between reads.
    pub fn read_cpu(&mut self) -> CpuMetrics {
        let current = Self::parse_proc_stat();
        let load_average = Self::parse_loadavg();
        let (processes, threads) = Self::parse_proc_stat_counts();

        let (total_percent, cores) = if let Some(ref prev) = self.prev_cpu_times {
            Self::compute_cpu_deltas(prev, &current)
        } else {
            (0.0, vec![])
        };

        self.prev_cpu_times = Some(current);

        CpuMetrics {
            cores,
            total_percent,
            load_average,
            processes,
            threads,
        }
    }

    /// Read memory metrics from /proc/meminfo.
    pub fn read_memory(&self) -> MemoryMetrics {
        let mut total = 0u64;
        let mut available = 0u64;
        let mut swap_total = 0u64;
        let mut swap_free = 0u64;

        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                    match parts[0] {
                        "MemTotal:" => total = kb,
                        "MemAvailable:" => available = kb,
                        "SwapTotal:" => swap_total = kb,
                        "SwapFree:" => swap_free = kb,
                        _ => {}
                    }
                }
            }
        }

        MemoryMetrics {
            total_bytes: total,
            used_bytes: total.saturating_sub(available),
            available_bytes: available,
            swap_total_bytes: swap_total,
            swap_used_bytes: swap_total.saturating_sub(swap_free),
            agent_usage: HashMap::new(),
        }
    }

    /// Read disk space metrics from /proc/mounts + statvfs.
    ///
    /// Filters to real filesystems (ext4, btrfs, xfs, f2fs, zfs, ntfs, vfat).
    pub fn read_disk(&self) -> Vec<DiskMetrics> {
        let mut disks = Vec::new();
        let real_fs = ["ext4", "ext3", "ext2", "btrfs", "xfs", "f2fs", "zfs", "ntfs", "vfat", "fuseblk"];

        let content = match std::fs::read_to_string("/proc/mounts") {
            Ok(c) => c,
            Err(_) => return disks,
        };

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            let device = parts[0];
            let mount_point = parts[1];
            let filesystem = parts[2];

            if !real_fs.contains(&filesystem) {
                continue;
            }

            if let Some(stat) = Self::statvfs(mount_point) {
                let total = stat.total_bytes;
                let available = stat.available_bytes;
                let used = total.saturating_sub(stat.free_bytes);

                disks.push(DiskMetrics {
                    mount_point: mount_point.to_string(),
                    device: device.to_string(),
                    filesystem: filesystem.to_string(),
                    total_bytes: total,
                    used_bytes: used,
                    available_bytes: available,
                    read_bytes: 0,
                    write_bytes: 0,
                });
            }
        }

        disks
    }

    /// Read network interface metrics from /proc/net/dev.
    pub fn read_network(&self) -> NetworkMetrics {
        let mut interfaces = Vec::new();
        let mut total_rx: u64 = 0;
        let mut total_tx: u64 = 0;

        if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                // Format: "iface: rx_bytes rx_packets rx_errs rx_drop ... tx_bytes tx_packets tx_errs tx_drop ..."
                let line = line.trim();
                let Some((name, rest)) = line.split_once(':') else {
                    continue;
                };
                let name = name.trim();
                let vals: Vec<u64> = rest
                    .split_whitespace()
                    .filter_map(|v| v.parse().ok())
                    .collect();

                if vals.len() < 16 {
                    continue;
                }

                let rx_bytes = vals[0];
                let rx_packets = vals[1];
                let rx_errors = vals[2];
                let tx_bytes = vals[8];
                let tx_packets = vals[9];
                let tx_errors = vals[10];

                // Skip loopback in totals
                if name != "lo" {
                    total_rx += rx_bytes;
                    total_tx += tx_bytes;
                }

                interfaces.push(InterfaceMetrics {
                    name: name.to_string(),
                    rx_bytes,
                    tx_bytes,
                    rx_packets,
                    tx_packets,
                    rx_errors,
                    tx_errors,
                    is_up: rx_bytes > 0 || tx_bytes > 0,
                });
            }
        }

        let active_connections = Self::count_connections();

        NetworkMetrics {
            interfaces,
            total_rx_bytes: total_rx,
            total_tx_bytes: total_tx,
            active_connections,
        }
    }

    /// Assemble a full SystemSnapshot from all local /proc readers.
    ///
    /// `agents` and `services` are passed in (they come from the daimon API,
    /// not from /proc).
    pub fn snapshot(&mut self, agents: AgentSummary, services: Vec<ServiceStatus>) -> SystemSnapshot {
        SystemSnapshot {
            timestamp: chrono::Utc::now(),
            cpu: self.read_cpu(),
            memory: self.read_memory(),
            disk: self.read_disk(),
            network: self.read_network(),
            agents,
            services,
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Parse all cpu lines from /proc/stat. Index 0 is the aggregate `cpu` line.
    fn parse_proc_stat() -> Vec<CpuTimes> {
        let content = match std::fs::read_to_string("/proc/stat") {
            Ok(c) => c,
            Err(_) => return vec![CpuTimes::default()],
        };

        let mut times = Vec::new();
        for line in content.lines() {
            if !line.starts_with("cpu") {
                continue;
            }
            // "cpu" (aggregate) or "cpu0", "cpu1", etc.
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 8 {
                continue;
            }
            let parse = |i: usize| -> u64 { parts.get(i).and_then(|v| v.parse().ok()).unwrap_or(0) };
            times.push(CpuTimes {
                user: parse(1),
                nice: parse(2),
                system: parse(3),
                idle: parse(4),
                iowait: parse(5),
                irq: parse(6),
                softirq: parse(7),
                steal: if parts.len() > 8 { parse(8) } else { 0 },
            });
        }

        if times.is_empty() {
            times.push(CpuTimes::default());
        }
        times
    }

    /// Parse /proc/loadavg.
    fn parse_loadavg() -> [f64; 3] {
        std::fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|s| {
                let parts: Vec<&str> = s.split_whitespace().collect();
                if parts.len() >= 3 {
                    Some([
                        parts[0].parse().unwrap_or(0.0),
                        parts[1].parse().unwrap_or(0.0),
                        parts[2].parse().unwrap_or(0.0),
                    ])
                } else {
                    None
                }
            })
            .unwrap_or([0.0; 3])
    }

    /// Parse process and thread counts from /proc/stat.
    fn parse_proc_stat_counts() -> (u64, u64) {
        let content = match std::fs::read_to_string("/proc/stat") {
            Ok(c) => c,
            Err(_) => return (0, 0),
        };

        let mut processes = 0u64;
        let mut procs_running = 0u64;

        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("processes ") {
                processes = rest.trim().parse().unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("procs_running ") {
                procs_running = rest.trim().parse().unwrap_or(0);
            }
        }

        (processes, procs_running)
    }

    /// Compute CPU usage percentages from two consecutive /proc/stat reads.
    fn compute_cpu_deltas(prev: &[CpuTimes], current: &[CpuTimes]) -> (f64, Vec<f64>) {
        let total_percent = if !prev.is_empty() && !current.is_empty() {
            let delta_total = current[0].total().saturating_sub(prev[0].total());
            let delta_busy = current[0].busy().saturating_sub(prev[0].busy());
            if delta_total == 0 {
                0.0
            } else {
                (delta_busy as f64 / delta_total as f64) * 100.0
            }
        } else {
            0.0
        };

        // Per-core: skip index 0 (aggregate), align prev[i] with current[i]
        let cores: Vec<f64> = (1..current.len())
            .map(|i| {
                if i < prev.len() {
                    let dt = current[i].total().saturating_sub(prev[i].total());
                    let db = current[i].busy().saturating_sub(prev[i].busy());
                    if dt == 0 { 0.0 } else { (db as f64 / dt as f64) * 100.0 }
                } else {
                    0.0
                }
            })
            .collect();

        (total_percent, cores)
    }

    /// Call libc::statvfs for a mount point.
    fn statvfs(path: &str) -> Option<StatVfsResult> {
        use std::ffi::CString;
        let c_path = CString::new(path).ok()?;
        unsafe {
            let mut buf: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(c_path.as_ptr(), &mut buf) == 0 {
                let block_size = buf.f_frsize;
                Some(StatVfsResult {
                    total_bytes: buf.f_blocks * block_size,
                    free_bytes: buf.f_bfree * block_size,
                    available_bytes: buf.f_bavail * block_size,
                })
            } else {
                None
            }
        }
    }

    /// Count established TCP connections from /proc/net/tcp.
    fn count_connections() -> u64 {
        let count_in = |path: &str| -> u64 {
            std::fs::read_to_string(path)
                .map(|c| {
                    c.lines()
                        .skip(1) // header
                        .filter(|line| {
                            // Column 4 (0-indexed 3) is the state; "01" = ESTABLISHED
                            line.split_whitespace()
                                .nth(3)
                                .is_some_and(|st| st == "01")
                        })
                        .count() as u64
                })
                .unwrap_or(0)
        };
        count_in("/proc/net/tcp") + count_in("/proc/net/tcp6")
    }
}

impl Default for ProcReader {
    fn default() -> Self {
        Self::new()
    }
}

struct StatVfsResult {
    total_bytes: u64,
    free_bytes: u64,
    available_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_reader_creates() {
        let reader = ProcReader::new();
        assert!(reader.prev_cpu_times.is_none());
    }

    #[test]
    fn read_cpu_first_call_zero() {
        let mut reader = ProcReader::new();
        let cpu = reader.read_cpu();
        // First read has no previous sample; total_percent should be 0
        assert_eq!(cpu.total_percent, 0.0);
        assert!(cpu.load_average[0] >= 0.0);
    }

    #[test]
    fn read_cpu_second_call_has_data() {
        let mut reader = ProcReader::new();
        let _first = reader.read_cpu();
        // On a real Linux system the second read should produce a percentage
        let second = reader.read_cpu();
        assert!(second.total_percent >= 0.0);
        assert!(second.total_percent <= 100.0);
    }

    #[test]
    fn read_memory_runs() {
        let reader = ProcReader::new();
        let m = reader.read_memory();
        assert!(m.used_bytes <= m.total_bytes);
    }

    #[test]
    fn read_disk_runs() {
        let reader = ProcReader::new();
        let disks = reader.read_disk();
        // Should find at least root filesystem on Linux
        if cfg!(target_os = "linux") {
            assert!(!disks.is_empty(), "expected at least one disk on Linux");
            for d in &disks {
                assert!(d.total_bytes > 0);
                assert!(d.used_bytes <= d.total_bytes);
            }
        }
    }

    #[test]
    fn read_network_runs() {
        let reader = ProcReader::new();
        let net = reader.read_network();
        // Should find at least lo on Linux
        if cfg!(target_os = "linux") {
            assert!(!net.interfaces.is_empty(), "expected at least one interface");
            assert!(net.interfaces.iter().any(|i| i.name == "lo"));
        }
    }

    #[test]
    fn snapshot_assembles() {
        let mut reader = ProcReader::new();
        let agents = AgentSummary {
            total: 0,
            running: 0,
            idle: 0,
            error: 0,
            cpu_usage: HashMap::new(),
            memory_usage: HashMap::new(),
        };
        let snap = reader.snapshot(agents, vec![]);
        assert!(snap.memory.total_bytes > 0 || !cfg!(target_os = "linux"));
    }

    #[test]
    fn compute_cpu_deltas_basic() {
        let prev = vec![CpuTimes {
            user: 100,
            nice: 0,
            system: 50,
            idle: 850,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
        }];
        let current = vec![CpuTimes {
            user: 200,
            nice: 0,
            system: 100,
            idle: 900,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
        }];
        let (total, cores) = ProcReader::compute_cpu_deltas(&prev, &current);
        // delta_total = 200, delta_busy = 150, percent = 75%
        assert!((total - 75.0).abs() < 0.01);
        assert!(cores.is_empty()); // no per-core entries
    }

    #[test]
    fn compute_cpu_deltas_with_cores() {
        let prev = vec![
            CpuTimes { user: 100, nice: 0, system: 50, idle: 850, iowait: 0, irq: 0, softirq: 0, steal: 0 },
            CpuTimes { user: 50, nice: 0, system: 25, idle: 425, iowait: 0, irq: 0, softirq: 0, steal: 0 },
        ];
        let current = vec![
            CpuTimes { user: 200, nice: 0, system: 100, idle: 900, iowait: 0, irq: 0, softirq: 0, steal: 0 },
            CpuTimes { user: 100, nice: 0, system: 50, idle: 450, iowait: 0, irq: 0, softirq: 0, steal: 0 },
        ];
        let (total, cores) = ProcReader::compute_cpu_deltas(&prev, &current);
        assert!((total - 75.0).abs() < 0.01);
        assert_eq!(cores.len(), 1);
        // core0 delta_total = 100, delta_busy = 75 → 75%
        assert!((cores[0] - 75.0).abs() < 0.01);
    }

    #[test]
    fn count_connections_runs() {
        // Should not panic
        let _ = ProcReader::count_connections();
    }
}
