//! Nazar API — Daimon HTTP client and /proc-based system metric readers.

mod proc_reader;

pub use proc_reader::ProcReader;

/// Client for the AGNOS daimon API.
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

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn health(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/health", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    pub async fn metrics(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/metrics", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    pub async fn agents(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/agents", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    pub async fn anomaly_alerts(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/anomaly/alerts", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    pub async fn scan_status(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/scan/status", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
    }

    pub async fn edge_dashboard(&self) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/v1/edge/dashboard", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json().await?;
        Ok(body)
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
}
