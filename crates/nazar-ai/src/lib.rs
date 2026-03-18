//! Nazar AI — anomaly detection, resource prediction, and recommendations

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use nazar_core::*;

/// Minimum seconds between duplicate alerts for the same component.
const ALERT_COOLDOWN_SECS: i64 = 60;

/// Simple threshold-based anomaly detector for system metrics.
pub struct AnomalyDetector {
    cpu_threshold: f64,
    memory_threshold: f64,
    disk_threshold: f64,
    history: VecDeque<SystemSnapshot>,
    max_history: usize,
    /// Tracks the last alert time per component for deduplication.
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

    /// Create a detector with thresholds from NazarConfig.
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

    /// Check a snapshot for anomalies. Deduplicates alerts per component
    /// with a 60-second cooldown.
    pub fn check(&mut self, snapshot: &SystemSnapshot) -> Vec<Alert> {
        let now = Utc::now();
        let mut alerts = Vec::new();

        if snapshot.cpu.total_percent > self.cpu_threshold {
            self.maybe_alert(&mut alerts, now, AlertSeverity::Warning, "cpu", format!(
                "CPU usage at {:.1}% (threshold: {:.1}%)",
                snapshot.cpu.total_percent, self.cpu_threshold
            ));
        }

        if snapshot.memory.used_percent() > self.memory_threshold {
            self.maybe_alert(&mut alerts, now, AlertSeverity::Warning, "memory", format!(
                "Memory usage at {:.1}% (threshold: {:.1}%)",
                snapshot.memory.used_percent(), self.memory_threshold
            ));
        }

        for disk in &snapshot.disk {
            if disk.used_percent() > self.disk_threshold {
                let component = format!("disk:{}", disk.mount_point);
                let message = format!(
                    "Disk {} at {:.1}% (threshold: {:.1}%)",
                    disk.mount_point, disk.used_percent(), self.disk_threshold
                );
                self.maybe_alert(&mut alerts, now, AlertSeverity::Critical, &component, message);
            }
        }

        alerts
    }

    /// Push an alert only if the component hasn't alerted within the cooldown period.
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

    /// Predict future resource usage based on linear trend.
    pub fn predict_memory_exhaustion(&self) -> Option<PredictionResult> {
        if self.history.len() < 10 {
            return None;
        }

        let points: Vec<(f64, f64)> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.memory.used_percent()))
            .collect();

        let (slope, _intercept) = linear_regression(&points)?;
        if slope <= 0.0 {
            return None;
        }

        let target = 95.0;
        let current_value = points.last().map(|p| p.1).unwrap_or(0.0);

        // Already past the target — exhaustion is now
        if current_value >= target {
            return Some(PredictionResult {
                metric: "memory".to_string(),
                current_value,
                predicted_value: target,
                intervals_until: 0,
                trend: Trend::Rising,
            });
        }

        // Calculate remaining intervals based on how far current_value is from
        // the target at the current slope, not from the regression intercept.
        let remaining = (target - current_value) / slope;

        // Negative or non-finite means exhaustion is not approaching
        if !remaining.is_finite() || remaining < 0.0 {
            return None;
        }

        // Cap at a reasonable maximum (7 days at 5s intervals = ~120,960)
        let intervals = (remaining as u64).min(200_000);

        Some(PredictionResult {
            metric: "memory".to_string(),
            current_value,
            predicted_value: target,
            intervals_until: intervals,
            trend: if slope > 0.5 {
                Trend::Rising
            } else {
                Trend::Stable
            },
        })
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple linear regression: returns (slope, intercept) for y = slope*x + intercept.
pub fn linear_regression(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    let n = points.len() as f64;
    if n < 2.0 {
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
    Some((slope, intercept))
}

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
            agents: AgentSummary {
                total: 3,
                running: 2,
                idle: 1,
                error: 0,
                cpu_usage: HashMap::new(),
                memory_usage: HashMap::new(),
            },
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
    }
}
