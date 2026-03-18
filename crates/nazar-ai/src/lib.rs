//! Nazar AI — anomaly detection, resource prediction, and correlation analysis

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use nazar_core::*;

/// Minimum seconds between duplicate alerts for the same component.
const ALERT_COOLDOWN_SECS: i64 = 60;

// ---------------------------------------------------------------------------
// Anomaly Detector
// ---------------------------------------------------------------------------

/// Simple threshold-based anomaly detector for system metrics.
pub struct AnomalyDetector {
    cpu_threshold: f64,
    memory_threshold: f64,
    disk_threshold: f64,
    history: VecDeque<SystemSnapshot>,
    max_history: usize,
    last_alert: HashMap<String, DateTime<Utc>>,
}

impl AnomalyDetector {
    pub fn new() -> Self {
        Self {
            cpu_threshold: 90.0,
            memory_threshold: 85.0,
            disk_threshold: 90.0,
            history: VecDeque::new(),
            max_history: 100,
            last_alert: HashMap::new(),
        }
    }

    pub fn from_config(config: &NazarConfig) -> Self {
        Self {
            cpu_threshold: config.cpu_threshold,
            memory_threshold: config.memory_threshold,
            disk_threshold: config.disk_threshold,
            history: VecDeque::new(),
            max_history: 100,
            last_alert: HashMap::new(),
        }
    }

    pub fn set_thresholds(&mut self, cpu: f64, memory: f64, disk: f64) {
        self.cpu_threshold = cpu;
        self.memory_threshold = memory;
        self.disk_threshold = disk;
    }

    pub fn record(&mut self, snapshot: SystemSnapshot) {
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(snapshot);
    }

    pub fn check(&mut self, snapshot: &SystemSnapshot) -> Vec<Alert> {
        let now = Utc::now();
        let mut alerts = Vec::new();

        if snapshot.cpu.total_percent > self.cpu_threshold {
            self.maybe_alert(
                &mut alerts,
                now,
                AlertSeverity::Warning,
                "cpu",
                format!(
                    "CPU usage at {:.1}% (threshold: {:.1}%)",
                    snapshot.cpu.total_percent, self.cpu_threshold
                ),
            );
        }

        if snapshot.memory.used_percent() > self.memory_threshold {
            self.maybe_alert(
                &mut alerts,
                now,
                AlertSeverity::Warning,
                "memory",
                format!(
                    "Memory usage at {:.1}% (threshold: {:.1}%)",
                    snapshot.memory.used_percent(),
                    self.memory_threshold
                ),
            );
        }

        for disk in &snapshot.disk {
            if disk.used_percent() > self.disk_threshold {
                let component = format!("disk:{}", disk.mount_point);
                let message = format!(
                    "Disk {} at {:.1}% (threshold: {:.1}%)",
                    disk.mount_point,
                    disk.used_percent(),
                    self.disk_threshold
                );
                self.maybe_alert(
                    &mut alerts,
                    now,
                    AlertSeverity::Critical,
                    &component,
                    message,
                );
            }
        }

        alerts
    }

    fn maybe_alert(
        &mut self,
        alerts: &mut Vec<Alert>,
        now: DateTime<Utc>,
        severity: AlertSeverity,
        component: &str,
        message: String,
    ) {
        if let Some(last) = self.last_alert.get(component)
            && (now - *last).num_seconds() < ALERT_COOLDOWN_SECS
        {
            return;
        }
        self.last_alert.insert(component.to_string(), now);
        alerts.push(Alert {
            severity,
            component: component.to_string(),
            message,
            timestamp: now,
        });
    }

    /// Predict exhaustion for all trackable metrics.
    pub fn predict_all(&self) -> Vec<PredictionResult> {
        if self.history.len() < 10 {
            return Vec::new();
        }

        let mut results = Vec::new();

        // Memory
        if let Some(p) = self.predict_metric("memory", 95.0, |s| s.memory.used_percent()) {
            results.push(p);
        }

        // CPU saturation
        if let Some(p) = self.predict_metric("cpu", 100.0, |s| s.cpu.total_percent) {
            results.push(p);
        }

        // Per-disk exhaustion
        if let Some(last) = self.history.back() {
            for disk in &last.disk {
                let mount = disk.mount_point.clone();
                if let Some(p) = self.predict_metric(&format!("disk:{mount}"), 95.0, |s| {
                    s.disk
                        .iter()
                        .find(|d| d.mount_point == mount)
                        .map(|d| d.used_percent())
                        .unwrap_or(0.0)
                }) {
                    results.push(p);
                }
            }
        }

        results
    }

    /// Generic prediction for a single metric with confidence intervals.
    fn predict_metric(
        &self,
        metric_name: &str,
        target: f64,
        extractor: impl Fn(&SystemSnapshot) -> f64,
    ) -> Option<PredictionResult> {
        let points: Vec<(f64, f64)> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, extractor(s)))
            .collect();

        let (slope, _intercept, se_slope) = linear_regression_with_se(&points)?;
        if slope <= 0.0 {
            return None;
        }

        let current_value = points.last().map(|p| p.1).unwrap_or(0.0);
        if current_value >= target {
            return Some(PredictionResult {
                metric: metric_name.to_string(),
                current_value,
                predicted_value: target,
                intervals_until: 0,
                trend: Trend::Rising,
                confidence_low: Some(0),
                confidence_high: Some(0),
            });
        }

        let remaining = (target - current_value) / slope;
        if !remaining.is_finite() || remaining < 0.0 {
            return None;
        }

        let intervals = (remaining as u64).min(200_000);

        // 95% confidence interval on slope: slope ± 1.96 * se_slope
        let (conf_low, conf_high) = if se_slope > 1e-10 {
            let slope_high = slope + 1.96 * se_slope;
            let slope_low = (slope - 1.96 * se_slope).max(0.001);
            let int_fast = ((target - current_value) / slope_high).max(0.0) as u64;
            let int_slow = ((target - current_value) / slope_low).min(200_000.0) as u64;
            (Some(int_fast), Some(int_slow))
        } else {
            (None, None)
        };

        Some(PredictionResult {
            metric: metric_name.to_string(),
            current_value,
            predicted_value: target,
            intervals_until: intervals,
            trend: if slope > 0.5 {
                Trend::Rising
            } else {
                Trend::Stable
            },
            confidence_low: conf_low,
            confidence_high: conf_high,
        })
    }

    /// Legacy method — delegates to predict_all.
    pub fn predict_memory_exhaustion(&self) -> Option<PredictionResult> {
        self.predict_all()
            .into_iter()
            .find(|p| p.metric == "memory")
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Correlation Detector
// ---------------------------------------------------------------------------

/// Tracks metric pairs and computes Pearson correlation coefficients.
pub struct CorrelationDetector {
    buffers: HashMap<&'static str, VecDeque<f64>>,
    max_samples: usize,
    pairs: Vec<(&'static str, &'static str)>,
}

impl CorrelationDetector {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            max_samples: 60,
            pairs: vec![
                ("cpu", "disk_io"),
                ("cpu", "network_tx"),
                ("memory", "swap"),
                ("memory", "network_rx"),
            ],
        }
    }

    pub fn record(&mut self, snapshot: &SystemSnapshot) {
        let metrics: &[(&str, f64)] = &[
            ("cpu", snapshot.cpu.total_percent),
            ("memory", snapshot.memory.used_percent()),
            ("swap", snapshot.memory.swap_used_percent()),
            ("network_rx", snapshot.network.total_rx_bytes as f64),
            ("network_tx", snapshot.network.total_tx_bytes as f64),
            (
                "disk_io",
                snapshot
                    .disk
                    .iter()
                    .map(|d| (d.read_bytes + d.write_bytes) as f64)
                    .sum(),
            ),
        ];

        for &(name, value) in metrics {
            let buf = self.buffers.entry(name).or_default();
            if buf.len() >= self.max_samples {
                buf.pop_front();
            }
            buf.push_back(value);
        }
    }

    pub fn compute(&self) -> Vec<CorrelationResult> {
        let mut results = Vec::new();

        for &(a, b) in &self.pairs {
            let Some(buf_a) = self.buffers.get(a) else {
                continue;
            };
            let Some(buf_b) = self.buffers.get(b) else {
                continue;
            };

            let n = buf_a.len().min(buf_b.len());
            if n < 10 {
                continue;
            }

            let xs: Vec<f64> = buf_a.iter().rev().take(n).copied().collect();
            let ys: Vec<f64> = buf_b.iter().rev().take(n).copied().collect();

            if let Some(r) = pearson(&xs, &ys) {
                let abs_r = r.abs();
                if abs_r < 0.4 {
                    continue;
                }
                let strength = if abs_r > 0.7 {
                    CorrelationStrength::Strong
                } else {
                    CorrelationStrength::Moderate
                };
                results.push(CorrelationResult {
                    metric_a: a.to_string(),
                    metric_b: b.to_string(),
                    coefficient: r,
                    strength,
                    sample_count: n,
                });
            }
        }

        results
    }
}

impl Default for CorrelationDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

/// Simple linear regression: returns (slope, intercept).
pub fn linear_regression(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    let (slope, intercept, _) = linear_regression_with_se(points)?;
    Some((slope, intercept))
}

/// Linear regression with standard error of slope.
pub fn linear_regression_with_se(points: &[(f64, f64)]) -> Option<(f64, f64, f64)> {
    let n = points.len() as f64;
    if n < 3.0 {
        return None;
    }
    let sum_x: f64 = points.iter().map(|p| p.0).sum();
    let sum_y: f64 = points.iter().map(|p| p.1).sum();
    let sum_xy: f64 = points.iter().map(|p| p.0 * p.1).sum();
    let sum_xx: f64 = points.iter().map(|p| p.0 * p.0).sum();

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-10 {
        return None;
    }
    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;

    // Standard error of the slope
    let x_mean = sum_x / n;
    let ss_xx: f64 = points.iter().map(|p| (p.0 - x_mean).powi(2)).sum();
    let residuals_sq: f64 = points
        .iter()
        .map(|p| {
            let pred = slope * p.0 + intercept;
            (p.1 - pred).powi(2)
        })
        .sum();
    let se_residuals = (residuals_sq / (n - 2.0)).max(0.0).sqrt();
    let se_slope = if ss_xx > 0.0 {
        se_residuals / ss_xx.sqrt()
    } else {
        0.0
    };

    Some((slope, intercept, se_slope))
}

/// Pearson correlation coefficient between two series.
pub fn pearson(xs: &[f64], ys: &[f64]) -> Option<f64> {
    let n = xs.len().min(ys.len());
    if n < 3 {
        return None;
    }
    let nf = n as f64;
    let sum_x: f64 = xs[..n].iter().sum();
    let sum_y: f64 = ys[..n].iter().sum();
    let sum_xy: f64 = xs[..n].iter().zip(&ys[..n]).map(|(x, y)| x * y).sum();
    let sum_xx: f64 = xs[..n].iter().map(|x| x * x).sum();
    let sum_yy: f64 = ys[..n].iter().map(|y| y * y).sum();

    let denom = ((nf * sum_xx - sum_x * sum_x) * (nf * sum_yy - sum_y * sum_y)).sqrt();
    if denom.abs() < 1e-10 {
        return None;
    }
    let r = (nf * sum_xy - sum_x * sum_y) / denom;
    if r.is_finite() { Some(r) } else { None }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_snapshot(cpu_pct: f64, mem_used: u64, mem_total: u64) -> SystemSnapshot {
        SystemSnapshot {
            timestamp: Utc::now(),
            cpu: CpuMetrics {
                cores: vec![cpu_pct],
                total_percent: cpu_pct,
                load_average: [1.0, 1.0, 1.0],
                processes: 100,
                threads: 500,
            },
            memory: MemoryMetrics {
                total_bytes: mem_total,
                used_bytes: mem_used,
                available_bytes: mem_total - mem_used,
                swap_total_bytes: 0,
                swap_used_bytes: 0,
                agent_usage: HashMap::new(),
            },
            disk: vec![DiskMetrics {
                mount_point: "/".to_string(),
                device: "/dev/sda1".to_string(),
                filesystem: "ext4".to_string(),
                total_bytes: 500_000_000_000,
                used_bytes: 250_000_000_000,
                available_bytes: 250_000_000_000,
                read_bytes: 0,
                write_bytes: 0,
            }],
            network: NetworkMetrics {
                interfaces: vec![],
                total_rx_bytes: 0,
                total_tx_bytes: 0,
                active_connections: 10,
            },
            agents: AgentSummary::default(),
            temperatures: vec![],
            gpu: vec![],
            services: vec![],
            top_processes: vec![],
        }
    }

    #[test]
    fn anomaly_detector_no_alerts() {
        let mut detector = AnomalyDetector::new();
        let snap = sample_snapshot(50.0, 8_000_000_000, 16_000_000_000);
        let alerts = detector.check(&snap);
        assert!(alerts.is_empty());
    }

    #[test]
    fn anomaly_detector_cpu_alert() {
        let mut detector = AnomalyDetector::new();
        let snap = sample_snapshot(95.0, 8_000_000_000, 16_000_000_000);
        let alerts = detector.check(&snap);
        assert!(alerts.iter().any(|a| a.component == "cpu"));
    }

    #[test]
    fn anomaly_detector_memory_alert() {
        let mut detector = AnomalyDetector::new();
        let snap = sample_snapshot(50.0, 14_000_000_000, 16_000_000_000);
        let alerts = detector.check(&snap);
        assert!(alerts.iter().any(|a| a.component == "memory"));
    }

    #[test]
    fn anomaly_detector_disk_alert() {
        let mut detector = AnomalyDetector::new();
        detector.set_thresholds(90.0, 85.0, 40.0);
        let snap = sample_snapshot(50.0, 8_000_000_000, 16_000_000_000);
        let alerts = detector.check(&snap);
        assert!(alerts.iter().any(|a| a.component.starts_with("disk:")));
    }

    #[test]
    fn anomaly_detector_custom_thresholds() {
        let mut detector = AnomalyDetector::new();
        detector.set_thresholds(50.0, 50.0, 40.0);
        let snap = sample_snapshot(60.0, 9_000_000_000, 16_000_000_000);
        let alerts = detector.check(&snap);
        assert!(alerts.iter().any(|a| a.component == "cpu"));
        assert!(alerts.iter().any(|a| a.component == "memory"));
        assert!(alerts.iter().any(|a| a.component.starts_with("disk:")));
    }

    #[test]
    fn linear_regression_basic() {
        let points = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
        let (slope, intercept) = linear_regression(&points).unwrap();
        assert!((slope - 1.0).abs() < 0.01);
        assert!(intercept.abs() < 0.01);
    }

    #[test]
    fn linear_regression_offset() {
        let points = vec![(0.0, 10.0), (1.0, 12.0), (2.0, 14.0)];
        let (slope, intercept) = linear_regression(&points).unwrap();
        assert!((slope - 2.0).abs() < 0.01);
        assert!((intercept - 10.0).abs() < 0.01);
    }

    #[test]
    fn linear_regression_with_se_returns_se() {
        let points = vec![(0.0, 0.0), (1.0, 1.1), (2.0, 1.9), (3.0, 3.1)];
        let (slope, _intercept, se) = linear_regression_with_se(&points).unwrap();
        assert!((slope - 1.0).abs() < 0.2);
        assert!(se > 0.0);
    }

    #[test]
    fn predict_not_enough_data() {
        let detector = AnomalyDetector::new();
        assert!(detector.predict_memory_exhaustion().is_none());
    }

    #[test]
    fn predict_rising_memory() {
        let mut detector = AnomalyDetector::new();
        for i in 0..20 {
            let used_pct = 60.0 + (i as f64);
            let used_bytes = (used_pct / 100.0 * 16_000_000_000.0) as u64;
            detector.record(sample_snapshot(50.0, used_bytes, 16_000_000_000));
        }
        let pred = detector.predict_memory_exhaustion();
        assert!(pred.is_some());
        let pred = pred.unwrap();
        assert_eq!(pred.metric, "memory");
        assert!(pred.intervals_until > 0);
        // Confidence may be None for perfectly linear data (zero SE)
    }

    #[test]
    fn predict_all_multi_metric() {
        let mut detector = AnomalyDetector::new();
        for i in 0..20 {
            let cpu = 60.0 + (i as f64) * 2.0; // rising CPU
            let mem_pct = 60.0 + (i as f64);
            let mem_bytes = (mem_pct / 100.0 * 16e9) as u64;
            detector.record(sample_snapshot(cpu, mem_bytes, 16_000_000_000));
        }
        let preds = detector.predict_all();
        assert!(preds.iter().any(|p| p.metric == "memory"));
        assert!(preds.iter().any(|p| p.metric == "cpu"));
    }

    #[test]
    fn pearson_perfect_correlation() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = pearson(&xs, &ys).unwrap();
        assert!((r - 1.0).abs() < 0.001);
    }

    #[test]
    fn pearson_anti_correlation() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        let r = pearson(&xs, &ys).unwrap();
        assert!((r + 1.0).abs() < 0.001);
    }

    #[test]
    fn pearson_not_enough_data() {
        assert!(pearson(&[1.0, 2.0], &[3.0, 4.0]).is_none());
    }

    #[test]
    fn correlation_detector_records_and_computes() {
        let mut detector = CorrelationDetector::new();
        // Feed correlated CPU + disk_io data
        for i in 0..20 {
            let mut snap = sample_snapshot(50.0 + i as f64, 8_000_000_000, 16_000_000_000);
            snap.disk[0].read_bytes = (i * 1000) as u64;
            detector.record(&snap);
        }
        let corrs = detector.compute();
        // cpu and disk_io should be correlated
        let cpu_disk = corrs
            .iter()
            .find(|c| c.metric_a == "cpu" && c.metric_b == "disk_io");
        assert!(cpu_disk.is_some());
        assert!(cpu_disk.unwrap().coefficient > 0.9);
    }
}
