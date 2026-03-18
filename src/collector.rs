//! Metrics collector — polls /proc and daimon, feeds anomaly detector.

use nazar_ai::AnomalyDetector;
use nazar_api::{ProcReader, ServiceChecker};
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

    // Take an initial reading so the next one can compute CPU and network deltas
    let _warmup = reader.read_cpu();
    let _warmup_net = reader.read_network();

    // Extract host from api_url for service checks
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

    // Check services and agents less frequently (every 6th tick = ~30s at default 5s poll)
    let mut tick_count: u64 = 0;
    let mut cached_services = Vec::new();
    let mut cached_agents = AgentSummary::default();

    loop {
        interval.tick().await;
        tick_count += 1;

        // Refresh config on each tick
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
        // Re-create interval outside the lock scope (tick().await is not Send with guard held)
        if poll_changed {
            interval = tokio::time::interval(std::time::Duration::from_secs(current_poll_secs));
            interval.tick().await; // consume the immediate first tick
            tracing::info!("Poll interval changed to {current_poll_secs}s");
        }

        // Probe AGNOS services and fetch agent data periodically
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

        // Feed the anomaly detector
        let alerts = if show_anomalies {
            detector.check(&snapshot)
        } else {
            Vec::new()
        };
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
            // Convert bytes-per-interval to bytes-per-second
            let poll = s.config.poll_interval_secs.max(1) as f64;
            s.net_rx_history.push(snapshot.network.total_rx_bytes as f64 / poll);
            s.net_tx_history.push(snapshot.network.total_tx_bytes as f64 / poll);

            let max = s.config.max_history_points;
            // Collect current mount points
            let current_mounts: std::collections::HashSet<&str> =
                snapshot.disk.iter().map(|d| d.mount_point.as_str()).collect();
            // Remove history for unmounted filesystems
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
            s.latest = Some(snapshot);
        }
    }
}
