//! Metrics collector — polls /proc and daimon, feeds anomaly detector.

use std::sync::{Arc, Mutex};

use nazar_ai::{AnomalyDetector, CorrelationDetector};
use nazar_api::{ProcReader, ServiceChecker};
use nazar_core::*;
use nazar_store::MetricStore;

/// Run the metrics collection loop. Reads system metrics, checks for anomalies,
/// probes AGNOS services, and writes everything to shared state.
pub async fn collector_loop(state: SharedState, store: Option<Arc<Mutex<MetricStore>>>) {
    let mut reader = ProcReader::new();

    let (mut detector, mut correlation_detector, poll_secs, api_url) = {
        let s = read_state(&state);
        let detector = AnomalyDetector::from_config(&s.config);
        let correlation_detector = CorrelationDetector::new();
        (detector, correlation_detector, s.config.poll_interval_secs, s.config.api_url.clone())
    };

    // Load historical snapshots into detectors if store available
    if let Some(ref store) = store
        && let Ok(s) = store.lock()
        && let Ok(snaps) = s.load_recent_snapshots(100)
    {
        for snap in snaps {
            correlation_detector.record(&snap);
            detector.record(snap);
        }
    }

    // Take an initial reading so the next one can compute CPU and network deltas
    let _warmup = reader.read_cpu();
    let _warmup_net = reader.read_network();

    let service_host = api_url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .unwrap_or("127.0.0.1");

    let service_checker = ServiceChecker::new(service_host);
    if service_checker.is_none() {
        tracing::warn!("Invalid service host '{service_host}', service checks disabled");
    }

    let mut current_poll_secs = poll_secs;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(current_poll_secs));
    tracing::info!("Collector started (poll every {current_poll_secs}s)");

    let mut tick_count: u64 = 0;
    let mut cached_services = Vec::new();
    let mut cached_agents = AgentSummary::default();

    loop {
        interval.tick().await;
        tick_count += 1;

        let show_anomalies;
        let mut poll_changed = false;
        {
            let s = read_state(&state);
            detector.set_thresholds(
                s.config.cpu_threshold,
                s.config.memory_threshold,
                s.config.disk_threshold,
            );
            show_anomalies = s.config.show_anomalies;

            if s.config.poll_interval_secs != current_poll_secs {
                current_poll_secs = s.config.poll_interval_secs;
                poll_changed = true;
            }
        }
        if poll_changed {
            interval = tokio::time::interval(std::time::Duration::from_secs(current_poll_secs));
            interval.tick().await;
            tracing::info!("Poll interval changed to {current_poll_secs}s");
        }

        if tick_count % 6 == 1
            && let Some(ref checker) = service_checker
        {
            cached_services = checker.check().await;
            cached_agents = checker.fetch_agents().await;
        }

        let top_n = {
            let s = read_state(&state);
            s.config.top_processes
        };
        let snapshot = reader.snapshot(cached_agents.clone(), cached_services.clone(), top_n);

        let alerts = if show_anomalies {
            detector.check(&snapshot)
        } else {
            Vec::new()
        };
        detector.record(snapshot.clone());
        correlation_detector.record(&snapshot);

        let predictions = detector.predict_all();
        let correlations = if tick_count.is_multiple_of(12) {
            correlation_detector.compute()
        } else {
            Vec::new()
        };

        // Persist to SQLite every ~1 min
        if tick_count.is_multiple_of(12)
            && let Some(ref store) = store
            && let Ok(s) = store.lock()
        {
            if let Err(e) = s.write_snapshot(&snapshot) {
                tracing::warn!("Failed to persist snapshot: {e}");
            }
            if !alerts.is_empty()
                && let Err(e) = s.write_alerts(&alerts)
            {
                tracing::warn!("Failed to persist alerts: {e}");
            }
            if !predictions.is_empty()
                && let Err(e) = s.write_predictions(&predictions)
            {
                tracing::warn!("Failed to persist predictions: {e}");
            }
        }

        // Write to shared state
        {
            let mut s = write_state(&state);
            s.cpu_history.push(snapshot.cpu.total_percent);
            s.mem_history.push(snapshot.memory.used_percent());
            let poll = s.config.poll_interval_secs.max(1) as f64;
            s.net_rx_history.push(snapshot.network.total_rx_bytes as f64 / poll);
            s.net_tx_history.push(snapshot.network.total_tx_bytes as f64 / poll);

            let max = s.config.max_history_points;
            for iface in &snapshot.network.interfaces {
                if iface.name == "lo" {
                    continue;
                }
                let (rx_hist, tx_hist) = s.net_iface_history
                    .entry(iface.name.clone())
                    .or_insert_with(|| (
                        TimeSeries::new(format!("{} RX", iface.name), "B/s", max),
                        TimeSeries::new(format!("{} TX", iface.name), "B/s", max),
                    ));
                rx_hist.push(iface.rx_bytes as f64 / poll);
                tx_hist.push(iface.tx_bytes as f64 / poll);
            }
            let current_ifaces: std::collections::HashSet<&str> =
                snapshot.network.interfaces.iter().map(|i| i.name.as_str()).collect();
            s.net_iface_history.retain(|k, _| current_ifaces.contains(k.as_str()));

            let current_mounts: std::collections::HashSet<&str> =
                snapshot.disk.iter().map(|d| d.mount_point.as_str()).collect();
            s.disk_history.retain(|k, _| current_mounts.contains(k.as_str()));
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
            if !correlations.is_empty() {
                s.correlations = correlations;
            }
            s.latest = Some(snapshot);
        }
    }
}
