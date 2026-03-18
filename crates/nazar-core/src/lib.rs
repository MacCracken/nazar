//! Nazar Core — Types and metrics for the AGNOS system monitor
//!
//! Named after the Arabic/Persian نظر (watchful eye).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// System metrics
// ---------------------------------------------------------------------------

/// A point-in-time system metrics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub timestamp: DateTime<Utc>,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: Vec<DiskMetrics>,
    pub network: NetworkMetrics,
    pub temperatures: Vec<ThermalInfo>,
    pub gpu: Vec<GpuMetrics>,
    pub agents: AgentSummary,
    pub services: Vec<ServiceStatus>,
    pub top_processes: Vec<ProcessInfo>,
}

/// GPU metrics from sysfs (amdgpu) or nvidia-smi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMetrics {
    pub id: String,
    pub driver: String,
    pub name: String,
    pub utilization_percent: f64,
    pub vram_total_bytes: u64,
    pub vram_used_bytes: u64,
    pub temp_celsius: Option<f64>,
    pub power_watts: Option<f64>,
    pub clock_mhz: Option<u64>,
}

impl GpuMetrics {
    pub fn vram_used_percent(&self) -> f64 {
        if self.vram_total_bytes == 0 {
            return 0.0;
        }
        (self.vram_used_bytes as f64 / self.vram_total_bytes as f64) * 100.0
    }
}

/// Temperature reading from a thermal zone or hardware monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalInfo {
    pub label: String,
    /// Temperature in degrees Celsius.
    pub temp_celsius: f64,
    /// Critical temperature threshold, if known.
    pub critical_celsius: Option<f64>,
}

/// Per-process resource usage (top-N by CPU).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub state: char,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub memory_percent: f64,
    pub threads: u64,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

// ---------------------------------------------------------------------------
// Alert types (used by nazar-ai, HTTP API, MCP, UI)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub severity: AlertSeverity,
    pub component: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub metric: String,
    pub current_value: f64,
    pub predicted_value: f64,
    /// How many polling intervals until the predicted value is reached.
    pub intervals_until: u64,
    pub trend: Trend,
    /// 95% confidence interval lower bound (intervals).
    pub confidence_low: Option<u64>,
    /// 95% confidence interval upper bound (intervals).
    pub confidence_high: Option<u64>,
}

/// Cross-metric correlation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationResult {
    pub metric_a: String,
    pub metric_b: String,
    /// Pearson correlation coefficient (-1.0 to 1.0).
    pub coefficient: f64,
    pub strength: CorrelationStrength,
    pub sample_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrelationStrength {
    Strong,
    Moderate,
    Weak,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trend {
    Rising,
    Stable,
    Falling,
}

/// Tool registration payload for daimon's MCP registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistration {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Time series
// ---------------------------------------------------------------------------

/// A time-series data point for charting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

/// Time-series buffer for a single metric (ring buffer via VecDeque).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    pub name: String,
    pub unit: String,
    pub points: VecDeque<DataPoint>,
    pub max_points: usize,
}

impl TimeSeries {
    pub fn new(name: impl Into<String>, unit: impl Into<String>, max_points: usize) -> Self {
        Self {
            name: name.into(),
            unit: unit.into(),
            points: VecDeque::new(),
            max_points,
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.max_points == 0 {
            return;
        }
        if self.points.len() >= self.max_points {
            self.points.pop_front();
        }
        self.points.push_back(DataPoint {
            timestamp: Utc::now(),
            value,
        });
    }

    pub fn latest(&self) -> Option<f64> {
        self.points.back().map(|p| p.value)
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

    /// Return the last `n` values (for sparklines / charts).
    pub fn last_n(&self, n: usize) -> Vec<f64> {
        let start = self.points.len().saturating_sub(n);
        self.points.iter().skip(start).map(|p| p.value).collect()
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

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
    /// CPU usage threshold for anomaly alerts (0.0–100.0).
    pub cpu_threshold: f64,
    /// Memory usage threshold for anomaly alerts (0.0–100.0).
    pub memory_threshold: f64,
    /// Disk usage threshold for anomaly alerts (0.0–100.0).
    pub disk_threshold: f64,
    /// Number of top processes to track by CPU usage.
    pub top_processes: usize,
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
            cpu_threshold: 90.0,
            memory_threshold: 85.0,
            disk_threshold: 90.0,
            top_processes: 10,
        }
    }
}

impl NazarConfig {
    /// Default config file path: `~/.config/nazar/config.json`.
    pub fn config_path() -> Option<PathBuf> {
        dirs_path().map(|d| d.join("config.json"))
    }

    /// Load config from the default path, falling back to defaults for missing fields.
    /// Returns `Default` if the file doesn't exist or can't be parsed.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        Self::load_from(&path)
    }

    /// Load config from a specific path.
    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse config from {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Save config to the default path. Creates parent directories if needed.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path()
            .ok_or_else(|| "Cannot determine config directory".to_string())?;
        self.save_to(&path)
    }

    /// Save config to a specific path.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        std::fs::write(path, json)
            .map_err(|e| format!("Failed to write config to {}: {e}", path.display()))
    }
}

/// Returns the nazar config directory: `~/.config/nazar/`.
fn dirs_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join("nazar"))
}

// ---------------------------------------------------------------------------
// Shared monitor state
// ---------------------------------------------------------------------------

/// Shared application state written by the collector and read by UI/API/MCP.
pub struct MonitorState {
    pub latest: Option<SystemSnapshot>,
    pub alerts: Vec<Alert>,
    pub predictions: Vec<PredictionResult>,
    pub correlations: Vec<CorrelationResult>,
    pub cpu_history: TimeSeries,
    pub mem_history: TimeSeries,
    pub disk_history: HashMap<String, TimeSeries>,
    pub net_rx_history: TimeSeries,
    pub net_tx_history: TimeSeries,
    /// Per-interface network history: interface_name -> (rx B/s, tx B/s).
    pub net_iface_history: HashMap<String, (TimeSeries, TimeSeries)>,
    pub config: NazarConfig,
    pub started_at: DateTime<Utc>,
    /// LLM triage explanation for the most recent alert.
    pub last_triage: Option<String>,
    /// LLM process recommendations.
    pub last_recommendations: Option<String>,
}

impl MonitorState {
    pub fn new(config: NazarConfig) -> Self {
        let max = config.max_history_points;
        Self {
            latest: None,
            alerts: Vec::new(),
            predictions: Vec::new(),
            correlations: Vec::new(),
            cpu_history: TimeSeries::new("CPU", "%", max),
            mem_history: TimeSeries::new("Memory", "%", max),
            disk_history: HashMap::new(),
            net_rx_history: TimeSeries::new("Net RX", "B/s", max),
            net_tx_history: TimeSeries::new("Net TX", "B/s", max),
            net_iface_history: HashMap::new(),
            config,
            started_at: Utc::now(),
            last_triage: None,
            last_recommendations: None,
        }
    }

    /// Cap alerts to the most recent 100.
    pub fn push_alerts(&mut self, new_alerts: Vec<Alert>) {
        self.alerts.extend(new_alerts);
        if self.alerts.len() > 100 {
            let drain = self.alerts.len() - 100;
            self.alerts.drain(..drain);
        }
    }
}

/// Thread-safe handle to shared monitor state.
pub type SharedState = Arc<RwLock<MonitorState>>;

pub fn new_shared_state(config: NazarConfig) -> SharedState {
    Arc::new(RwLock::new(MonitorState::new(config)))
}

/// Read from shared state, recovering from a poisoned lock instead of panicking.
/// A poisoned lock means a writer panicked mid-update — state may be inconsistent.
pub fn read_state(state: &SharedState) -> std::sync::RwLockReadGuard<'_, MonitorState> {
    state.read().unwrap_or_else(|poisoned| {
        tracing::error!("RwLock poisoned (read) — a writer panicked; state may be inconsistent");
        poisoned.into_inner()
    })
}

/// Write to shared state, recovering from a poisoned lock instead of panicking.
/// A poisoned lock means a previous writer panicked mid-update — state may be inconsistent.
pub fn write_state(state: &SharedState) -> std::sync::RwLockWriteGuard<'_, MonitorState> {
    state.write().unwrap_or_else(|poisoned| {
        tracing::error!("RwLock poisoned (write) — a writer panicked; state may be inconsistent");
        poisoned.into_inner()
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    fn time_series_zero_max_points() {
        let mut ts = TimeSeries::new("zero", "", 0);
        ts.push(42.0);
        ts.push(99.0);
        assert!(ts.points.is_empty());
        assert_eq!(ts.latest(), None);
    }

    #[test]
    fn time_series_last_n() {
        let mut ts = TimeSeries::new("test", "", 10);
        for i in 0..5 {
            ts.push(i as f64);
        }
        assert_eq!(ts.last_n(3), vec![2.0, 3.0, 4.0]);
        assert_eq!(ts.last_n(10), vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn default_config() {
        let cfg = NazarConfig::default();
        assert_eq!(cfg.poll_interval_secs, 5);
        assert_eq!(cfg.max_history_points, 720);
        assert!(cfg.show_anomalies);
        assert!((cfg.cpu_threshold - 90.0).abs() < f64::EPSILON);
        assert!((cfg.memory_threshold - 85.0).abs() < f64::EPSILON);
        assert!((cfg.disk_threshold - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn alert_severity_display() {
        assert_eq!(AlertSeverity::Info.to_string(), "INFO");
        assert_eq!(AlertSeverity::Warning.to_string(), "WARNING");
        assert_eq!(AlertSeverity::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn config_save_and_load() {
        let dir = std::env::temp_dir().join("nazar-test-config");
        let path = dir.join("config.json");

        let mut cfg = NazarConfig::default();
        cfg.cpu_threshold = 75.0;
        cfg.poll_interval_secs = 10;
        cfg.save_to(&path).unwrap();

        let loaded = NazarConfig::load_from(&path);
        assert!((loaded.cpu_threshold - 75.0).abs() < f64::EPSILON);
        assert_eq!(loaded.poll_interval_secs, 10);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_load_missing_file_returns_default() {
        let cfg = NazarConfig::load_from(std::path::Path::new("/nonexistent/config.json"));
        assert_eq!(cfg.poll_interval_secs, 5);
    }

    #[test]
    fn config_load_invalid_json_returns_default() {
        let dir = std::env::temp_dir().join("nazar-test-bad-config");
        let path = dir.join("config.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "not valid json {{{").unwrap();

        let cfg = NazarConfig::load_from(&path);
        assert_eq!(cfg.poll_interval_secs, 5);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn monitor_state_push_alerts_caps() {
        let mut state = MonitorState::new(NazarConfig::default());
        for i in 0..150 {
            state.push_alerts(vec![Alert {
                severity: AlertSeverity::Info,
                component: format!("test-{i}"),
                message: "test".to_string(),
                timestamp: Utc::now(),
            }]);
        }
        assert_eq!(state.alerts.len(), 100);
        // Oldest should be trimmed — latest component is "test-149"
        assert_eq!(state.alerts.last().unwrap().component, "test-149");
    }
}
