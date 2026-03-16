//! Nazar Core — Types and metrics for the AGNOS system monitor
//!
//! Named after the Arabic/Persian نظر (watchful eye).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A point-in-time system metrics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub timestamp: DateTime<Utc>,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: Vec<DiskMetrics>,
    pub network: NetworkMetrics,
    pub agents: AgentSummary,
    pub services: Vec<ServiceStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    /// Per-core usage (0.0–100.0).
    pub cores: Vec<f64>,
    /// Overall usage (0.0–100.0).
    pub total_percent: f64,
    /// Load average (1m, 5m, 15m).
    pub load_average: [f64; 3],
    /// Number of running processes.
    pub processes: u64,
    /// Number of threads.
    pub threads: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    /// Per-agent memory usage.
    pub agent_usage: HashMap<String, u64>,
}

impl MemoryMetrics {
    pub fn used_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f64 / self.total_bytes as f64) * 100.0
    }

    pub fn swap_used_percent(&self) -> f64 {
        if self.swap_total_bytes == 0 {
            return 0.0;
        }
        (self.swap_used_bytes as f64 / self.swap_total_bytes as f64) * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub mount_point: String,
    pub device: String,
    pub filesystem: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    /// I/O read bytes since last snapshot.
    pub read_bytes: u64,
    /// I/O write bytes since last snapshot.
    pub write_bytes: u64,
}

impl DiskMetrics {
    pub fn used_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f64 / self.total_bytes as f64) * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub interfaces: Vec<InterfaceMetrics>,
    /// Total bytes received since last snapshot.
    pub total_rx_bytes: u64,
    /// Total bytes transmitted since last snapshot.
    pub total_tx_bytes: u64,
    /// Active connections count.
    pub active_connections: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceMetrics {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub is_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub total: usize,
    pub running: usize,
    pub idle: usize,
    pub error: usize,
    /// Per-agent CPU usage (agent_id -> percent).
    pub cpu_usage: HashMap<String, f64>,
    /// Per-agent memory usage (agent_id -> bytes).
    pub memory_usage: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub state: ServiceState,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceState {
    Running,
    Stopped,
    Failed,
    Starting,
    Unknown,
}

impl std::fmt::Display for ServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed => write!(f, "failed"),
            Self::Starting => write!(f, "starting"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// An anomaly alert from the daimon anomaly detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    pub agent_id: String,
    pub metric: String,
    pub severity: String,
    pub current_value: f64,
    pub baseline_mean: f64,
    pub deviation_sigmas: f64,
    pub timestamp: DateTime<Utc>,
}

/// A time-series data point for charting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

/// Time-series buffer for a single metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    pub name: String,
    pub unit: String,
    pub points: Vec<DataPoint>,
    pub max_points: usize,
}

impl TimeSeries {
    pub fn new(name: impl Into<String>, unit: impl Into<String>, max_points: usize) -> Self {
        Self {
            name: name.into(),
            unit: unit.into(),
            points: Vec::new(),
            max_points,
        }
    }

    pub fn push(&mut self, value: f64) {
        self.points.push(DataPoint {
            timestamp: Utc::now(),
            value,
        });
        if self.points.len() > self.max_points {
            self.points.remove(0);
        }
    }

    pub fn latest(&self) -> Option<f64> {
        self.points.last().map(|p| p.value)
    }

    pub fn average(&self) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.points.iter().map(|p| p.value).sum();
        sum / self.points.len() as f64
    }

    pub fn min(&self) -> Option<f64> {
        self.points.iter().map(|p| p.value).reduce(f64::min)
    }

    pub fn max(&self) -> Option<f64> {
        self.points.iter().map(|p| p.value).reduce(f64::max)
    }
}

/// Nazar configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NazarConfig {
    /// Daimon API URL.
    pub api_url: String,
    /// Polling interval in seconds.
    pub poll_interval_secs: u64,
    /// Maximum data points to retain per metric.
    pub max_history_points: usize,
    /// Whether to show anomaly alerts.
    pub show_anomalies: bool,
    /// Whether to show per-agent breakdown.
    pub show_agents: bool,
    /// Refresh rate for the UI (ms).
    pub ui_refresh_ms: u64,
}

impl Default for NazarConfig {
    fn default() -> Self {
        Self {
            api_url: "http://127.0.0.1:8090".to_string(),
            poll_interval_secs: 5,
            max_history_points: 720, // 1 hour at 5s intervals
            show_anomalies: true,
            show_agents: true,
            ui_refresh_ms: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_metrics_percent() {
        let m = MemoryMetrics {
            total_bytes: 16_000_000_000,
            used_bytes: 8_000_000_000,
            available_bytes: 8_000_000_000,
            swap_total_bytes: 4_000_000_000,
            swap_used_bytes: 1_000_000_000,
            agent_usage: HashMap::new(),
        };
        assert!((m.used_percent() - 50.0).abs() < 0.01);
        assert!((m.swap_used_percent() - 25.0).abs() < 0.01);
    }

    #[test]
    fn memory_metrics_zero_total() {
        let m = MemoryMetrics {
            total_bytes: 0,
            used_bytes: 0,
            available_bytes: 0,
            swap_total_bytes: 0,
            swap_used_bytes: 0,
            agent_usage: HashMap::new(),
        };
        assert_eq!(m.used_percent(), 0.0);
        assert_eq!(m.swap_used_percent(), 0.0);
    }

    #[test]
    fn disk_metrics_percent() {
        let d = DiskMetrics {
            mount_point: "/".to_string(),
            device: "/dev/sda1".to_string(),
            filesystem: "ext4".to_string(),
            total_bytes: 500_000_000_000,
            used_bytes: 250_000_000_000,
            available_bytes: 250_000_000_000,
            read_bytes: 0,
            write_bytes: 0,
        };
        assert!((d.used_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn service_state_display() {
        assert_eq!(ServiceState::Running.to_string(), "running");
        assert_eq!(ServiceState::Failed.to_string(), "failed");
        assert_eq!(ServiceState::Unknown.to_string(), "unknown");
    }

    #[test]
    fn time_series_push_and_query() {
        let mut ts = TimeSeries::new("cpu", "%", 5);
        ts.push(10.0);
        ts.push(20.0);
        ts.push(30.0);
        assert_eq!(ts.latest(), Some(30.0));
        assert!((ts.average() - 20.0).abs() < 0.01);
        assert_eq!(ts.min(), Some(10.0));
        assert_eq!(ts.max(), Some(30.0));
    }

    #[test]
    fn time_series_max_points() {
        let mut ts = TimeSeries::new("mem", "bytes", 3);
        for i in 0..5 {
            ts.push(i as f64);
        }
        assert_eq!(ts.points.len(), 3);
        assert_eq!(ts.latest(), Some(4.0));
    }

    #[test]
    fn time_series_empty() {
        let ts = TimeSeries::new("empty", "", 10);
        assert_eq!(ts.latest(), None);
        assert_eq!(ts.average(), 0.0);
        assert_eq!(ts.min(), None);
        assert_eq!(ts.max(), None);
    }

    #[test]
    fn default_config() {
        let cfg = NazarConfig::default();
        assert_eq!(cfg.poll_interval_secs, 5);
        assert_eq!(cfg.max_history_points, 720);
        assert!(cfg.show_anomalies);
    }
}
