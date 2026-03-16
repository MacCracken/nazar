//! Nazar API Client — connects to daimon (port 8090) for agent/health/anomaly data
//! and reads /proc for local system metrics.

use nazar_core::*;
use std::collections::HashMap;

/// Client for the AGNOS daimon API and local system metrics.
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Fetch health status from daimon.
    pub async fn health(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/health", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    /// Fetch metrics from daimon.
    pub async fn metrics(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/metrics", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    /// Fetch registered agents.
    pub async fn agents(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/agents", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    /// Fetch anomaly alerts.
    pub async fn anomaly_alerts(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/anomaly/alerts", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    /// Fetch phylax scan status.
    pub async fn scan_status(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/scan/status", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    /// Fetch edge fleet dashboard.
    pub async fn edge_dashboard(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/edge/dashboard", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    /// Read local CPU metrics from /proc/stat.
    pub fn read_cpu_metrics() -> CpuMetrics {
        // Read /proc/loadavg for load averages
        let load_average = std::fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|s| {
                let parts: Vec<&str> = s.split_whitespace().collect();
                if parts.len() >= 3 {
                    Some([
                        parts[0].parse::<f64>().unwrap_or(0.0),
                        parts[1].parse::<f64>().unwrap_or(0.0),
                        parts[2].parse::<f64>().unwrap_or(0.0),
                    ])
                } else {
                    None
                }
            })
            .unwrap_or([0.0; 3]);

        CpuMetrics {
            cores: vec![],
            total_percent: 0.0, // requires delta calculation between two reads
            load_average,
            processes: 0,
            threads: 0,
        }
    }

    /// Read local memory metrics from /proc/meminfo.
    pub fn read_memory_metrics() -> MemoryMetrics {
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
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Failed to read system metrics: {0}")]
    System(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_client_new() {
        let client = ApiClient::new("http://127.0.0.1:8090");
        assert_eq!(client.base_url, "http://127.0.0.1:8090");
    }

    #[test]
    fn api_client_strips_trailing_slash() {
        let client = ApiClient::new("http://127.0.0.1:8090/");
        assert_eq!(client.base_url, "http://127.0.0.1:8090");
    }

    #[test]
    fn read_memory_metrics_runs() {
        // Should not panic even if /proc/meminfo doesn't exist (e.g. macOS)
        let m = ApiClient::read_memory_metrics();
        // On Linux, total should be > 0; on other platforms, 0 is fine
        assert!(m.used_bytes <= m.total_bytes);
    }

    #[test]
    fn read_cpu_metrics_runs() {
        let c = ApiClient::read_cpu_metrics();
        // load_average should be non-negative
        assert!(c.load_average[0] >= 0.0);
    }
}
