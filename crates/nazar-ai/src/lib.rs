//! Nazar AI — anomaly detection, resource prediction, and recommendations

use chrono::Utc;
use nazar_core::*;

/// Simple threshold-based anomaly detector for system metrics.
pub struct AnomalyDetector {
    cpu_threshold: f64,
    memory_threshold: f64,
    disk_threshold: f64,
    history: Vec<SystemSnapshot>,
    max_history: usize,
}

impl AnomalyDetector {
    pub fn new() -> Self {
        Self {
            cpu_threshold: 90.0,
            memory_threshold: 85.0,
            disk_threshold: 90.0,
            history: Vec::new(),
            max_history: 100,
        }
    }

    /// Create a detector with thresholds from NazarConfig.
    pub fn from_config(config: &NazarConfig) -> Self {
        Self {
            cpu_threshold: config.cpu_threshold,
            memory_threshold: config.memory_threshold,
            disk_threshold: config.disk_threshold,
            history: Vec::new(),
            max_history: 100,
        }
    }

    pub fn set_thresholds(&mut self, cpu: f64, memory: f64, disk: f64) {
        self.cpu_threshold = cpu;
        self.memory_threshold = memory;
        self.disk_threshold = disk;
    }

    pub fn record(&mut self, snapshot: SystemSnapshot) {
        self.history.push(snapshot);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Check a snapshot for anomalies.
    pub fn check(&self, snapshot: &SystemSnapshot) -> Vec<Alert> {
        let mut alerts = Vec::new();

        if snapshot.cpu.total_percent > self.cpu_threshold {
            alerts.push(Alert {
                severity: AlertSeverity::Warning,
                component: "cpu".to_string(),
                message: format!(
                    "CPU usage at {:.1}% (threshold: {:.1}%)",
                    snapshot.cpu.total_percent, self.cpu_threshold
                ),
                timestamp: Utc::now(),
            });
        }

        if snapshot.memory.used_percent() > self.memory_threshold {
            alerts.push(Alert {
                severity: AlertSeverity::Warning,
                component: "memory".to_string(),
                message: format!(
                    "Memory usage at {:.1}% (threshold: {:.1}%)",
                    snapshot.memory.used_percent(),
                    self.memory_threshold
                ),
                timestamp: Utc::now(),
            });
        }

        for disk in &snapshot.disk {
            if disk.used_percent() > self.disk_threshold {
                alerts.push(Alert {
                    severity: AlertSeverity::Critical,
                    component: format!("disk:{}", disk.mount_point),
                    message: format!(
                        "Disk {} at {:.1}% (threshold: {:.1}%)",
                        disk.mount_point,
                        disk.used_percent(),
                        self.disk_threshold
                    ),
                    timestamp: Utc::now(),
                });
            }
        }

        alerts
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

        let (slope, intercept) = linear_regression(&points)?;
        if slope <= 0.0 {
            return None;
        }

        let target = 95.0;
        let steps_to_target = (target - intercept) / slope;
        let intervals = steps_to_target as u64;

        Some(PredictionResult {
            metric: "memory".to_string(),
            current_value: points.last().map(|p| p.1).unwrap_or(0.0),
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
            services: vec![],
        }
    }

    #[test]
    fn anomaly_detector_no_alerts() {
        let detector = AnomalyDetector::new();
        let snap = sample_snapshot(50.0, 8_000_000_000, 16_000_000_000);
        let alerts = detector.check(&snap);
        assert!(alerts.is_empty());
    }

    #[test]
    fn anomaly_detector_cpu_alert() {
        let detector = AnomalyDetector::new();
        let snap = sample_snapshot(95.0, 8_000_000_000, 16_000_000_000);
        let alerts = detector.check(&snap);
        assert!(alerts.iter().any(|a| a.component == "cpu"));
    }

    #[test]
    fn anomaly_detector_memory_alert() {
        let detector = AnomalyDetector::new();
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
