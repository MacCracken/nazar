//! Metrics collector — polls /proc and daimon, feeds anomaly detector.

use std::collections::HashMap;

use nazar_ai::AnomalyDetector;
use nazar_api::{ProcReader, check_services};
use nazar_core::*;

/// Run the metrics collection loop. Reads system metrics, checks for anomalies,
/// probes AGNOS services, and writes everything to shared state.
pub async fn collector_loop(state: SharedState) {
    let mut reader = ProcReader::new();

    let (mut detector, poll_secs, api_url) = {
        let s = read_state(&state);
        let detector = AnomalyDetector::from_config(&s.config);
        (detector, s.config.poll_interval_secs, s.config.api_url.clone())
    };

    // Take an initial reading so the next one can compute CPU deltas
    let _warmup = reader.read_cpu();

    // Extract host from api_url for service checks
    let service_host = api_url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .unwrap_or("127.0.0.1")
        .to_string();

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(poll_secs));
    tracing::info!("Collector started (poll every {poll_secs}s)");

    // Check services less frequently (every 6th tick = ~30s at default 5s poll)
    let mut tick_count: u64 = 0;
    let mut cached_services = Vec::new();

    loop {
        interval.tick().await;
        tick_count += 1;

        // Refresh thresholds from config on each tick
        {
            let s = read_state(&state);
            detector.set_thresholds(
                s.config.cpu_threshold,
                s.config.memory_threshold,
                s.config.disk_threshold,
            );
        }

        // Probe AGNOS services periodically
        if tick_count % 6 == 1 {
            cached_services = check_services(&service_host).await;
        }

        let agents = AgentSummary {
            total: 0,
            running: 0,
            idle: 0,
            error: 0,
            cpu_usage: HashMap::new(),
            memory_usage: HashMap::new(),
        };

        let snapshot = reader.snapshot(agents, cached_services.clone());

        // Feed the anomaly detector
        let alerts = detector.check(&snapshot);
        detector.record(snapshot.clone());

        // Update predictions
        let predictions = detector
            .predict_memory_exhaustion()
            .into_iter()
            .collect::<Vec<_>>();

        // Write to shared state
        {
            let mut s = write_state(&state);
            s.cpu_history.push(snapshot.cpu.total_percent);
            s.mem_history.push(snapshot.memory.used_percent());
            s.net_rx_history.push(snapshot.network.total_rx_bytes as f64);
            s.net_tx_history.push(snapshot.network.total_tx_bytes as f64);

            let max = s.config.max_history_points;
            for disk in &snapshot.disk {
                s.disk_history
                    .entry(disk.mount_point.clone())
                    .or_insert_with(|| TimeSeries::new(&disk.mount_point, "%", max))
                    .push(disk.used_percent());
            }

            if !alerts.is_empty() {
                tracing::warn!("{} alert(s) detected", alerts.len());
                for a in &alerts {
                    tracing::warn!("[{}] {}: {}", a.severity, a.component, a.message);
                }
                s.push_alerts(alerts);
            }

            s.predictions = predictions;
            s.latest = Some(snapshot);
        }
    }
}
