//! Probes known AGNOS services to build live ServiceStatus entries.

use nazar_core::{ServiceState, ServiceStatus};

/// Known AGNOS services and their default ports.
const KNOWN_SERVICES: &[(&str, u16)] = &[
    ("daimon", 8090),
    ("hoosh", 8088),
];

/// Probe each known service's health endpoint and return status entries.
pub async fn check_services(base_host: &str) -> Vec<ServiceStatus> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut statuses = Vec::new();

    for &(name, port) in KNOWN_SERVICES {
        let url = format!("http://{base_host}:{port}/v1/health");
        let status = match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let mut svc = ServiceStatus {
                    name: name.to_string(),
                    state: ServiceState::Running,
                    pid: None,
                    uptime_secs: None,
                    port: Some(port),
                };

                // Try to extract uptime from JSON response
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    svc.uptime_secs = body
                        .get("uptime_secs")
                        .and_then(|v| v.as_u64());
                    svc.pid = body
                        .get("pid")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32);
                }

                svc
            }
            Ok(_resp) => ServiceStatus {
                name: name.to_string(),
                state: ServiceState::Failed,
                pid: None,
                uptime_secs: None,
                port: Some(port),
            },
            Err(_) => ServiceStatus {
                name: name.to_string(),
                state: ServiceState::Stopped,
                pid: None,
                uptime_secs: None,
                port: Some(port),
            },
        };

        statuses.push(status);
    }

    statuses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_services_defined() {
        assert!(KNOWN_SERVICES.len() >= 2);
        assert!(KNOWN_SERVICES.iter().any(|(n, _)| *n == "daimon"));
        assert!(KNOWN_SERVICES.iter().any(|(n, _)| *n == "hoosh"));
    }

    #[tokio::test]
    async fn check_services_returns_entries() {
        // Even with no services running, should return status entries
        let statuses = check_services("127.0.0.1").await;
        assert_eq!(statuses.len(), KNOWN_SERVICES.len());
        for s in &statuses {
            assert!(s.port.is_some());
        }
    }
}
