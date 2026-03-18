//! Probes known AGNOS services to build live ServiceStatus entries.

use nazar_core::{AgentSummary, ServiceState, ServiceStatus};

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

    /// Fetch agent summary from daimon's `/v1/agents` endpoint.
    /// Returns `AgentSummary::default()` if daimon is unreachable or the response
    /// doesn't match the expected schema.
    pub async fn fetch_agents(&self) -> AgentSummary {
        let url = format!("http://{}:8090/v1/agents", self.host);
        let resp = match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => return AgentSummary::default(),
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return AgentSummary::default(),
        };

        let get_usize = |key| body.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        let mut summary = AgentSummary {
            total: get_usize("total"),
            running: get_usize("running"),
            idle: get_usize("idle"),
            error: get_usize("error"),
            ..AgentSummary::default()
        };

        // Parse per-agent CPU usage: {"cpu_usage": {"agent_id": 12.5, ...}}
        if let Some(cpu_map) = body.get("cpu_usage").and_then(|v| v.as_object()) {
            for (k, v) in cpu_map {
                if let Some(pct) = v.as_f64() {
                    summary.cpu_usage.insert(k.clone(), pct);
                }
            }
        }

        // Parse per-agent memory usage: {"memory_usage": {"agent_id": 1234567, ...}}
        if let Some(mem_map) = body.get("memory_usage").and_then(|v| v.as_object()) {
            for (k, v) in mem_map {
                if let Some(bytes) = v.as_u64() {
                    summary.memory_usage.insert(k.clone(), bytes);
                }
            }
        }

        summary
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

    #[tokio::test]
    async fn fetch_agents_returns_default_when_unreachable() {
        let checker = ServiceChecker::new("127.0.0.1").unwrap();
        let agents = checker.fetch_agents().await;
        // Daimon likely not running, should return defaults
        assert_eq!(agents.total, 0);
        assert_eq!(agents.running, 0);
        assert!(agents.cpu_usage.is_empty());
    }
}
