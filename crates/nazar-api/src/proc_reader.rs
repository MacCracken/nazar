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

/// Previous network counters for delta computation.
#[derive(Debug, Clone, Default)]
struct NetCounters {
    per_interface: HashMap<String, (u64, u64)>, // (rx_bytes, tx_bytes)
    total_rx: u64,
    total_tx: u64,
}

/// Per-process CPU time counters from /proc/[pid]/stat.
#[derive(Debug, Clone, Default)]
struct ProcCpuTimes {
    utime: u64,
    stime: u64,
}

impl ProcCpuTimes {
    fn total(&self) -> u64 {
        self.utime + self.stime
    }
}

/// Previous disk I/O counters for delta computation.
#[derive(Debug, Clone, Default)]
struct DiskIoCounters {
    /// device_name -> (read_bytes, write_bytes)
    per_device: HashMap<String, (u64, u64)>,
}

/// Reads system metrics from /proc on Linux.
///
/// Holds previous CPU, network, disk I/O, and per-process readings for delta-based metrics.
pub struct ProcReader {
    prev_cpu_times: Option<Vec<CpuTimes>>,
    prev_net: Option<NetCounters>,
    prev_disk_io: Option<DiskIoCounters>,
    prev_proc_times: HashMap<u32, ProcCpuTimes>,
    /// System-wide total CPU delta from the last `read_cpu()` call.
    last_system_cpu_delta: u64,
    num_cores: usize,
    page_size: u64,
    /// Total threads from last `read_processes()` call.
    total_threads: u64,
}

impl ProcReader {
    pub fn new() -> Self {
        // SAFETY: sysconf(_SC_PAGESIZE) is always safe and returns a valid value.
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        Self {
            prev_cpu_times: None,
            prev_net: None,
            prev_disk_io: None,
            prev_proc_times: HashMap::new(),
            last_system_cpu_delta: 0,
            num_cores: 0,
            page_size,
            total_threads: 0,
        }
    }

    /// Read CPU metrics from /proc/stat and /proc/loadavg.
    ///
    /// The first call returns 0% usage (no previous sample to diff against).
    /// Subsequent calls return real usage based on the delta between reads.
    pub fn read_cpu(&mut self) -> CpuMetrics {
        let (current, procs_running) = Self::parse_proc_stat();
        let load_average = Self::parse_loadavg();

        let (total_percent, cores) = if let Some(ref prev) = self.prev_cpu_times {
            // Store system-wide CPU delta for per-process CPU% calculation
            self.last_system_cpu_delta = current[0].total().saturating_sub(prev[0].total());
            Self::compute_cpu_deltas(prev, &current)
        } else {
            self.last_system_cpu_delta = 0;
            (0.0, vec![])
        };

        self.num_cores = if current.len() > 1 { current.len() - 1 } else { 1 };
        self.prev_cpu_times = Some(current);

        CpuMetrics {
            cores,
            total_percent,
            load_average,
            processes: procs_running,
            threads: 0,
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

    /// Read disk space metrics from /proc/mounts + statvfs, with I/O throughput
    /// from /proc/diskstats (delta-based).
    pub fn read_disk(&mut self) -> Vec<DiskMetrics> {
        // First, read current I/O counters from /proc/diskstats
        let current_io = Self::parse_diskstats();

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
            let mount_point = Self::decode_octal_escapes(parts[1]);
            let filesystem = parts[2];

            if !real_fs.contains(&filesystem) {
                continue;
            }

            if let Some(stat) = Self::statvfs(&mount_point) {
                let total = stat.total_bytes;
                let available = stat.available_bytes;
                let used = total.saturating_sub(stat.free_bytes);

                // Extract short device name (e.g. "/dev/sda1" -> "sda1")
                let dev_short = device.rsplit('/').next().unwrap_or(device);

                // Compute I/O deltas
                let (read_bytes, write_bytes) = if let Some((cur_r, cur_w)) = current_io.per_device.get(dev_short) {
                    if let Some(ref prev) = self.prev_disk_io {
                        if let Some((prev_r, prev_w)) = prev.per_device.get(dev_short) {
                            (cur_r.saturating_sub(*prev_r), cur_w.saturating_sub(*prev_w))
                        } else {
                            (0, 0)
                        }
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                };

                disks.push(DiskMetrics {
                    mount_point,
                    device: device.to_string(),
                    filesystem: filesystem.to_string(),
                    total_bytes: total,
                    used_bytes: used,
                    available_bytes: available,
                    read_bytes,
                    write_bytes,
                });
            }
        }

        self.prev_disk_io = Some(current_io);
        disks
    }

    /// Read temperature sensors from /sys/class/thermal and /sys/class/hwmon.
    pub fn read_temperatures(&self) -> Vec<ThermalInfo> {
        let mut temps = Vec::new();

        // /sys/class/thermal/thermal_zone*/temp (millidegrees C)
        if let Ok(entries) = std::fs::read_dir("/sys/class/thermal") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.starts_with("thermal_zone") {
                    continue;
                }
                let path = entry.path();
                let temp = std::fs::read_to_string(path.join("temp"))
                    .ok()
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .map(|t| t as f64 / 1000.0);
                let label = std::fs::read_to_string(path.join("type"))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| name_str.to_string());
                let critical = std::fs::read_to_string(path.join("trip_point_0_temp"))
                    .ok()
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .map(|t| t as f64 / 1000.0);

                if let Some(temp_celsius) = temp {
                    temps.push(ThermalInfo {
                        label,
                        temp_celsius,
                        critical_celsius: critical,
                    });
                }
            }
        }

        // /sys/class/hwmon/hwmon*/temp*_input (millidegrees C)
        if let Ok(hwmons) = std::fs::read_dir("/sys/class/hwmon") {
            for hwmon in hwmons.flatten() {
                let hwmon_path = hwmon.path();
                let hwmon_name = std::fs::read_to_string(hwmon_path.join("name"))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();

                let Ok(files) = std::fs::read_dir(&hwmon_path) else { continue };
                for file in files.flatten() {
                    let fname = file.file_name();
                    let fname_str = fname.to_string_lossy();
                    if !fname_str.ends_with("_input") || !fname_str.starts_with("temp") {
                        continue;
                    }

                    let temp = std::fs::read_to_string(file.path())
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .map(|t| t as f64 / 1000.0);

                    let Some(temp_celsius) = temp else { continue };

                    // Try to read the label (e.g. temp1_label)
                    let label_file = fname_str.replace("_input", "_label");
                    let label = std::fs::read_to_string(hwmon_path.join(&label_file))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| {
                            if hwmon_name.is_empty() {
                                fname_str.replace("_input", "").to_string()
                            } else {
                                format!("{}/{}", hwmon_name, fname_str.replace("_input", ""))
                            }
                        });

                    // Try to read critical threshold
                    let crit_file = fname_str.replace("_input", "_crit");
                    let critical = std::fs::read_to_string(hwmon_path.join(&crit_file))
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .map(|t| t as f64 / 1000.0);

                    temps.push(ThermalInfo {
                        label,
                        temp_celsius,
                        critical_celsius: critical,
                    });
                }
            }
        }

        temps
    }

    /// Read network interface metrics from /proc/net/dev.
    ///
    /// Returns delta-based byte counts (bytes since last read) rather than
    /// cumulative counters. The first call returns zeros for deltas.
    pub fn read_network(&mut self) -> NetworkMetrics {
        let mut interfaces = Vec::new();
        let mut current_counters = NetCounters::default();

        if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
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

                let cum_rx = vals[0];
                let rx_packets = vals[1];
                let rx_errors = vals[2];
                let cum_tx = vals[8];
                let tx_packets = vals[9];
                let tx_errors = vals[10];

                // Compute deltas from previous reading
                let (delta_rx, delta_tx) = if let Some(ref prev) = self.prev_net {
                    if let Some(&(prev_rx, prev_tx)) = prev.per_interface.get(name) {
                        (cum_rx.saturating_sub(prev_rx), cum_tx.saturating_sub(prev_tx))
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                };

                current_counters.per_interface.insert(name.to_string(), (cum_rx, cum_tx));

                if name != "lo" {
                    current_counters.total_rx += cum_rx;
                    current_counters.total_tx += cum_tx;
                }

                // Check operstate for accurate up/down detection
                let is_up = std::fs::read_to_string(format!("/sys/class/net/{name}/operstate"))
                    .map(|s| s.trim() == "up")
                    .unwrap_or(cum_rx > 0 || cum_tx > 0);

                interfaces.push(InterfaceMetrics {
                    name: name.to_string(),
                    rx_bytes: delta_rx,
                    tx_bytes: delta_tx,
                    rx_packets,
                    tx_packets,
                    rx_errors,
                    tx_errors,
                    is_up,
                });
            }
        }

        let (total_delta_rx, total_delta_tx) = if let Some(ref prev) = self.prev_net {
            (
                current_counters.total_rx.saturating_sub(prev.total_rx),
                current_counters.total_tx.saturating_sub(prev.total_tx),
            )
        } else {
            (0, 0)
        };

        self.prev_net = Some(current_counters);

        let active_connections = Self::count_connections();

        NetworkMetrics {
            interfaces,
            total_rx_bytes: total_delta_rx,
            total_tx_bytes: total_delta_tx,
            active_connections,
        }
    }

    /// Read top-N processes by CPU usage from /proc/[pid]/stat.
    ///
    /// Uses delta-based CPU calculation. First call returns 0% for all processes.
    pub fn read_processes(&mut self, top_n: usize, total_mem_bytes: u64) -> Vec<ProcessInfo> {
        if top_n == 0 {
            return Vec::new();
        }

        let Ok(proc_dir) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };

        struct RawProc {
            pid: u32,
            name: String,
            state: char,
            utime: u64,
            stime: u64,
            num_threads: u64,
        }

        let mut current_procs = Vec::new();

        for entry in proc_dir.flatten() {
            let name = entry.file_name();
            let Some(pid_str) = name.to_str() else { continue };
            let Ok(pid) = pid_str.parse::<u32>() else { continue };

            let stat_path = format!("/proc/{pid}/stat");
            let Ok(content) = std::fs::read_to_string(&stat_path) else { continue };

            // Parse comm field: find first '(' and last ')' to handle names with parens
            let Some(comm_start) = content.find('(') else { continue };
            let Some(comm_end) = content.rfind(')') else { continue };
            if comm_end <= comm_start { continue; }

            let proc_name = content[comm_start + 1..comm_end].to_string();
            let rest = &content[comm_end + 2..]; // skip ") "
            let fields: Vec<&str> = rest.split_whitespace().collect();
            // fields[0] = state, fields[11] = utime, fields[12] = stime, fields[17] = num_threads
            if fields.len() < 18 { continue; }

            let state = fields[0].chars().next().unwrap_or('?');
            let utime = fields[11].parse::<u64>().unwrap_or(0);
            let stime = fields[12].parse::<u64>().unwrap_or(0);
            let num_threads = fields[17].parse::<u64>().unwrap_or(0);

            current_procs.push(RawProc { pid, name: proc_name, state, utime, stime, num_threads });
        }

        // Sum total threads across all processes
        self.total_threads = current_procs.iter().map(|p| p.num_threads).sum();

        // Compute CPU deltas for all processes, track all times for next tick
        let sys_delta = self.last_system_cpu_delta as f64;
        let mut new_prev = HashMap::with_capacity(current_procs.len());
        let mut procs_with_cpu: Vec<ProcessInfo> = current_procs
            .into_iter()
            .map(|raw| {
                let current_times = ProcCpuTimes { utime: raw.utime, stime: raw.stime };
                let cpu_percent = if sys_delta > 0.0 {
                    if let Some(prev) = self.prev_proc_times.get(&raw.pid) {
                        let proc_delta = current_times.total().saturating_sub(prev.total());
                        (proc_delta as f64 / sys_delta) * self.num_cores as f64 * 100.0
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                new_prev.insert(raw.pid, current_times);

                ProcessInfo {
                    pid: raw.pid,
                    name: raw.name,
                    state: raw.state,
                    cpu_percent,
                    memory_bytes: 0,
                    memory_percent: 0.0,
                    threads: raw.num_threads,
                }
            })
            .collect();

        self.prev_proc_times = new_prev;

        // Sort by CPU descending, take top-N
        procs_with_cpu.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));
        procs_with_cpu.truncate(top_n);

        // Read memory only for top-N (read /proc/[pid]/statm)
        for proc in &mut procs_with_cpu {
            let statm_path = format!("/proc/{}/statm", proc.pid);
            if let Ok(content) = std::fs::read_to_string(&statm_path)
                && let Some(rss_str) = content.split_whitespace().nth(1)
                && let Ok(rss_pages) = rss_str.parse::<u64>()
            {
                proc.memory_bytes = rss_pages * self.page_size;
                if total_mem_bytes > 0 {
                    proc.memory_percent = (proc.memory_bytes as f64 / total_mem_bytes as f64) * 100.0;
                }
            }
        }

        procs_with_cpu
    }

    /// Assemble a full SystemSnapshot from all local /proc readers.
    ///
    /// `agents` and `services` are passed in (they come from the daimon API,
    /// not from /proc).
    pub fn snapshot(&mut self, agents: AgentSummary, services: Vec<ServiceStatus>, top_n: usize) -> SystemSnapshot {
        let mut cpu = self.read_cpu();
        let memory = self.read_memory();
        let top_processes = self.read_processes(top_n, memory.total_bytes);
        cpu.threads = self.total_threads;
        SystemSnapshot {
            timestamp: chrono::Utc::now(),
            cpu,
            memory,
            disk: self.read_disk(),
            network: self.read_network(),
            temperatures: self.read_temperatures(),
            agents,
            services,
            top_processes,
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Parse /proc/diskstats for per-device I/O counters.
    /// Returns cumulative read/write bytes per device.
    fn parse_diskstats() -> DiskIoCounters {
        let mut counters = DiskIoCounters::default();
        let Ok(content) = std::fs::read_to_string("/proc/diskstats") else {
            return counters;
        };

        for line in content.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            // /proc/diskstats format: major minor name rd_ios ... rd_sectors ... wr_ios ... wr_sectors ...
            // Field indices (0-based): 2=name, 5=rd_sectors, 9=wr_sectors
            if fields.len() < 14 {
                continue;
            }
            let name = fields[2];
            // Skip partition-less whole-disk entries if partitions exist
            // (e.g. skip "sda" if "sda1" exists — we'll handle this by matching device names)
            let rd_sectors = fields[5].parse::<u64>().unwrap_or(0);
            let wr_sectors = fields[9].parse::<u64>().unwrap_or(0);
            // Sector size is 512 bytes in /proc/diskstats
            counters.per_device.insert(name.to_string(), (rd_sectors * 512, wr_sectors * 512));
        }

        counters
    }

    /// Parse /proc/stat in a single read: CPU times + running process count.
    fn parse_proc_stat() -> (Vec<CpuTimes>, u64) {
        let content = match std::fs::read_to_string("/proc/stat") {
            Ok(c) => c,
            Err(_) => return (vec![CpuTimes::default()], 0),
        };

        let mut times = Vec::new();
        let mut procs_running = 0u64;

        for line in content.lines() {
            if line.starts_with("cpu") {
                // Match "cpu " (aggregate) or "cpu0", "cpu1", etc.
                let first_word = line.split_whitespace().next().unwrap_or("");
                if first_word != "cpu" && !first_word.strip_prefix("cpu").is_some_and(|s| s.starts_with(|c: char| c.is_ascii_digit())) {
                    continue;
                }
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
            } else if let Some(rest) = line.strip_prefix("procs_running ") {
                procs_running = rest.trim().parse().unwrap_or(0);
            }
        }

        if times.is_empty() {
            times.push(CpuTimes::default());
        }
        (times, procs_running)
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

    /// Decode octal escape sequences in /proc/mounts paths (e.g. `\040` → space).
    fn decode_octal_escapes(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                let octal: String = chars.by_ref().take(3).collect();
                if octal.len() == 3
                    && let Ok(byte) = u8::from_str_radix(&octal, 8)
                {
                    result.push(byte as char);
                    continue;
                }
                // Not a valid octal escape, put it back as-is
                result.push('\\');
                result.push_str(&octal);
            } else {
                result.push(c);
            }
        }
        result
    }

    /// Call libc::statvfs for a mount point.
    fn statvfs(path: &str) -> Option<StatVfsResult> {
        use std::ffi::CString;
        let c_path = CString::new(path).ok()?;
        // SAFETY: `c_path` is a valid NUL-terminated C string (CString guarantees this),
        // and `buf` is a properly sized, zeroed statvfs struct passed by mutable pointer.
        // `libc::statvfs` writes into `buf` only on success (returns 0).
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
        let mut reader = ProcReader::new();
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
        let mut reader = ProcReader::new();
        let net = reader.read_network();
        // Should find at least lo on Linux
        if cfg!(target_os = "linux") {
            assert!(!net.interfaces.is_empty(), "expected at least one interface");
            assert!(net.interfaces.iter().any(|i| i.name == "lo"));
        }
    }

    #[test]
    fn read_network_deltas() {
        let mut reader = ProcReader::new();
        // First read: deltas should be zero (no previous sample)
        let first = reader.read_network();
        assert_eq!(first.total_rx_bytes, 0);
        assert_eq!(first.total_tx_bytes, 0);
        // Second read: deltas should be >= 0
        let second = reader.read_network();
        // Can't assert specific values, but should not panic
        assert!(second.total_rx_bytes <= u64::MAX);
    }

    #[test]
    fn snapshot_assembles() {
        let mut reader = ProcReader::new();
        let snap = reader.snapshot(AgentSummary::default(), vec![], 5);
        assert!(snap.memory.total_bytes > 0 || !cfg!(target_os = "linux"));
    }

    #[test]
    fn read_disk_io_deltas() {
        let mut reader = ProcReader::new();
        // First read: I/O deltas should be zero
        let first = reader.read_disk();
        for d in &first {
            assert_eq!(d.read_bytes, 0);
            assert_eq!(d.write_bytes, 0);
        }
        // Second read: deltas should be >= 0
        let second = reader.read_disk();
        for d in &second {
            assert!(d.read_bytes <= u64::MAX);
            assert!(d.write_bytes <= u64::MAX);
        }
    }

    #[test]
    fn read_temperatures_runs() {
        let reader = ProcReader::new();
        let temps = reader.read_temperatures();
        // On Linux with thermal zones, should find at least one
        // But don't hard-fail on systems without sensors
        for t in &temps {
            assert!(!t.label.is_empty());
            assert!(t.temp_celsius > -50.0 && t.temp_celsius < 200.0);
        }
    }

    #[test]
    fn parse_diskstats_runs() {
        let counters = ProcReader::parse_diskstats();
        if cfg!(target_os = "linux") {
            assert!(!counters.per_device.is_empty(), "expected at least one device");
        }
    }

    #[test]
    fn read_processes_returns_entries() {
        let mut reader = ProcReader::new();
        reader.read_cpu(); // prime system CPU delta
        let procs = reader.read_processes(10, 16_000_000_000);
        if cfg!(target_os = "linux") {
            assert!(!procs.is_empty(), "expected at least one process on Linux");
            for p in &procs {
                assert!(p.pid > 0);
                assert!(!p.name.is_empty());
            }
        }
    }

    #[test]
    fn read_processes_respects_top_n() {
        let mut reader = ProcReader::new();
        reader.read_cpu();
        let procs = reader.read_processes(3, 16_000_000_000);
        assert!(procs.len() <= 3);
    }

    #[test]
    fn read_processes_second_call_has_cpu() {
        let mut reader = ProcReader::new();
        reader.read_cpu();
        let _ = reader.read_processes(10, 16_000_000_000);
        // Second read with fresh CPU delta
        reader.read_cpu();
        let procs = reader.read_processes(10, 16_000_000_000);
        // At least one process should have >= 0 CPU (can't assert > 0 deterministically)
        for p in &procs {
            assert!(p.cpu_percent >= 0.0);
            assert!(p.cpu_percent <= 100.0 * reader.num_cores as f64);
        }
    }

    #[test]
    fn read_processes_has_memory() {
        let mut reader = ProcReader::new();
        reader.read_cpu();
        let procs = reader.read_processes(5, 16_000_000_000);
        if cfg!(target_os = "linux") {
            // At least one process should have non-zero memory
            assert!(procs.iter().any(|p| p.memory_bytes > 0));
        }
    }

    #[test]
    fn decode_octal_escapes_space() {
        assert_eq!(ProcReader::decode_octal_escapes("/mnt/my\\040drive"), "/mnt/my drive");
    }

    #[test]
    fn decode_octal_escapes_no_escapes() {
        assert_eq!(ProcReader::decode_octal_escapes("/home/user"), "/home/user");
    }

    #[test]
    fn decode_octal_escapes_tab() {
        assert_eq!(ProcReader::decode_octal_escapes("a\\011b"), "a\tb");
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
