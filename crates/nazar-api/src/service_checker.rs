//! Probes known AGNOS services and integrates with daimon/hoosh APIs.

use nazar_core::{AgentSummary, Alert, ServiceState, ServiceStatus};
use serde_json::json;

/// Known AGNOS services and their default ports.
const KNOWN_SERVICES: &[(&str, u16)] = &[("daimon", 8090), ("hoosh", 8088)];

/// Reusable AGNOS service integration with a shared HTTP client.
pub struct ServiceChecker {
    client: reqwest::Client,
    host: String,
}

impl ServiceChecker {
    /// Create a new checker for the given host.
    pub fn new(host: &str) -> Option<Self> {
        let host = host.trim();
        if host.is_empty() || host.contains('/') || host.contains(' ') || host.contains(':') {
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

    pub fn host(&self) -> &str {
        &self.host
    }

    // -----------------------------------------------------------------------
    // Service health checks
    // -----------------------------------------------------------------------

    /// Probe each known service's health endpoint.
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
                        svc.uptime_secs = body.get("uptime_secs").and_then(|v| v.as_u64());
                        svc.pid = body.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32);
                    }
                    svc
                }
                Ok(_) => ServiceStatus {
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

    // -----------------------------------------------------------------------
    // Agent data from daimon
    // -----------------------------------------------------------------------

    /// Fetch agent summary from daimon's `/v1/agents` endpoint.
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

        if let Some(cpu_map) = body.get("cpu_usage").and_then(|v| v.as_object()) {
            for (k, v) in cpu_map {
                if let Some(pct) = v.as_f64() {
                    summary.cpu_usage.insert(k.clone(), pct);
                }
            }
        }

        if let Some(mem_map) = body.get("memory_usage").and_then(|v| v.as_object()) {
            for (k, v) in mem_map {
                if let Some(bytes) = v.as_u64() {
                    summary.memory_usage.insert(k.clone(), bytes);
                }
            }
        }

        summary
    }

    // -----------------------------------------------------------------------
    // MCP tool registration in daimon
    // -----------------------------------------------------------------------

    /// Register nazar's MCP tools with daimon's tool registry.
    /// `callback_base` is e.g. `http://127.0.0.1:8095`.
    pub async fn register_mcp_tools(
        &self,
        tools: &[nazar_core::ToolRegistration],
        callback_base: &str,
    ) -> usize {
        let url = format!("http://{}:8090/v1/mcp/tools", self.host);
        let mut registered = 0;

        for tool in tools {
            let body = json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema,
                "callback_url": format!("{}/v1/mcp/call", callback_base),
                "source": "nazar",
            });

            match self.client.post(&url).json(&body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    registered += 1;
                    tracing::info!("Registered MCP tool: {}", tool.name);
                }
                Ok(resp) => {
                    tracing::debug!(
                        "Failed to register tool {}: HTTP {}",
                        tool.name,
                        resp.status()
                    );
                }
                Err(e) => {
                    tracing::debug!("Failed to register tool {}: {e}", tool.name);
                }
            }
        }

        registered
    }

    // -----------------------------------------------------------------------
    // LLM-assisted alert triage via hoosh
    // -----------------------------------------------------------------------

    /// Send an alert to hoosh for NL explanation.
    /// Returns the LLM's explanation or None if hoosh is unavailable.
    pub async fn triage_alert(&self, alert: &Alert) -> Option<String> {
        let url = format!("http://{}:8088/v1/chat/completions", self.host);

        let prompt = format!(
            "You are a system monitoring assistant for AGNOS. \
             Explain this alert concisely and suggest a fix:\n\n\
             Severity: {}\nComponent: {}\nMessage: {}",
            alert.severity, alert.component, alert.message
        );

        let body = json!({
            "model": "default",
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 200,
            "temperature": 0.3,
        });

        let resp = self
            .client
            .post(&url)
            .header("X-Source-Service", "nazar")
            .json(&body)
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let body: serde_json::Value = resp.json().await.ok()?;
        body.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
    }

    /// Send process data to hoosh for recommendations.
    pub async fn get_process_recommendations(
        &self,
        processes: &[nazar_core::ProcessInfo],
        memory_percent: f64,
        cpu_percent: f64,
    ) -> Option<String> {
        let url = format!("http://{}:8088/v1/chat/completions", self.host);

        let proc_summary: String = processes
            .iter()
            .take(5)
            .map(|p| {
                format!(
                    "  {} (PID {}): CPU {:.1}%, Mem {:.1} MB",
                    p.name,
                    p.pid,
                    p.cpu_percent,
                    p.memory_bytes as f64 / 1e6
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "You are a system monitoring assistant. Analyze these top processes and provide \
             brief recommendations (2-3 sentences max):\n\n\
             System: CPU {cpu_percent:.1}%, Memory {memory_percent:.1}%\n\
             Top processes:\n{proc_summary}"
        );

        let body = json!({
            "model": "default",
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 200,
            "temperature": 0.3,
        });

        let resp = self
            .client
            .post(&url)
            .header("X-Source-Service", "nazar")
            .json(&body)
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let body: serde_json::Value = resp.json().await.ok()?;
        body.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
    }

    // -----------------------------------------------------------------------
    // Alert notifications via daimon event bus
    // -----------------------------------------------------------------------

    /// Publish alerts to daimon's event bus for desktop notification.
    pub async fn publish_alerts(&self, alerts: &[Alert]) {
        if alerts.is_empty() {
            return;
        }

        let url = format!("http://{}:8090/v1/events/publish", self.host);

        for alert in alerts {
            let body = json!({
                "topic": "nazar.alerts",
                "data": {
                    "severity": alert.severity.to_string(),
                    "component": alert.component,
                    "message": alert.message,
                    "timestamp": alert.timestamp.to_rfc3339(),
                }
            });

            if let Err(e) = self.client.post(&url).json(&body).send().await {
                tracing::debug!("Failed to publish alert event: {e}");
                break; // daimon likely down, don't spam
            }
        }
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
        assert_eq!(agents.total, 0);
        assert!(agents.cpu_usage.is_empty());
    }

    #[tokio::test]
    async fn triage_alert_returns_none_when_unreachable() {
        let checker = ServiceChecker::new("127.0.0.1").unwrap();
        let alert = Alert {
            severity: nazar_core::AlertSeverity::Warning,
            component: "cpu".to_string(),
            message: "high cpu".to_string(),
            timestamp: chrono::Utc::now(),
        };
        assert!(checker.triage_alert(&alert).await.is_none());
    }

    #[tokio::test]
    async fn register_mcp_tools_returns_zero_when_unreachable() {
        let checker = ServiceChecker::new("127.0.0.1").unwrap();
        let tools = vec![nazar_core::ToolRegistration {
            name: "test".to_string(),
            description: "test tool".to_string(),
            input_schema: serde_json::json!({}),
        }];
        let count = checker
            .register_mcp_tools(&tools, "http://127.0.0.1:8095")
            .await;
        assert_eq!(count, 0);
    }
}
