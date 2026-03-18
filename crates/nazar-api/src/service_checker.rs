//! Probes known AGNOS services to build live ServiceStatus entries.

use nazar_core::{ServiceState, ServiceStatus};

/// Known AGNOS services and their default ports.
const KNOWN_SERVICES: &[(&str, u16)] = &[
    ("daimon", 8090),
    ("hoosh", 8088),
];

/// Reusable service health checker with a shared HTTP client.
pub struct ServiceChecker {
    client: reqwest::Client,
    host: String,
}

impl ServiceChecker {
    /// Create a new checker for the given host. Validates that `host` looks
    /// like a valid IP or hostname (no scheme, no path, no port).
    pub fn new(host: &str) -> Option<Self> {
        let host = host.trim();
        // Reject empty, hosts with schemes, paths, or spaces
        if host.is_empty()
            || host.contains('/')
            || host.contains(' ')
            || host.contains(':')
        {
            return None;
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Some(Self {
            client,
            host: host.to_string(),
        })
    }

    /// Probe each known service's health endpoint and return status entries.
    pub async fn check(&self) -> Vec<ServiceStatus> {
        let mut statuses = Vec::new();

        for &(name, port) in KNOWN_SERVICES {
            let url = format!("http://{}:{}/v1/health", self.host, port);
            let status = match self.client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let mut svc = ServiceStatus {
                        name: name.to_string(),
                        state: ServiceState::Running,
                        pid: None,
                        uptime_secs: None,
                        port: Some(port),
                    };

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

    #[test]
    fn valid_host_accepted() {
        assert!(ServiceChecker::new("127.0.0.1").is_some());
        assert!(ServiceChecker::new("localhost").is_some());
    }

    #[test]
    fn invalid_host_rejected() {
        assert!(ServiceChecker::new("").is_none());
        assert!(ServiceChecker::new("http://localhost").is_none());
        assert!(ServiceChecker::new("host:8090").is_none());
        assert!(ServiceChecker::new("host/path").is_none());
    }

    #[tokio::test]
    async fn check_services_returns_entries() {
        let checker = ServiceChecker::new("127.0.0.1").unwrap();
        let statuses = checker.check().await;
        assert_eq!(statuses.len(), KNOWN_SERVICES.len());
        for s in &statuses {
            assert!(s.port.is_some());
        }
    }
}
