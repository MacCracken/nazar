//! Nazar API — /proc-based system metric readers and AGNOS service probing.

mod proc_reader;
mod service_checker;

pub use proc_reader::ProcReader;
pub use service_checker::ServiceChecker;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Failed to read system metrics: {0}")]
    System(String),
}

#[cfg(test)]
mod tests {
    #[test]
    fn api_error_display() {
        let err = super::ApiError::System("test".to_string());
        assert_eq!(err.to_string(), "Failed to read system metrics: test");
    }
}
