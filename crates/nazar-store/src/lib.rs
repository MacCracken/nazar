//! Nazar Store — SQLite persistence for metrics, alerts, and predictions.

use std::path::Path;

use nazar_core::{Alert, PredictionResult, SystemSnapshot};
use rusqlite::{Connection, params};

const SCHEMA_DDL: &str = "
    CREATE TABLE IF NOT EXISTS snapshots (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp TEXT NOT NULL,
        data TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS alerts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp TEXT NOT NULL,
        severity TEXT NOT NULL,
        component TEXT NOT NULL,
        message TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS predictions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp TEXT NOT NULL,
        data TEXT NOT NULL
    );
";

pub struct MetricStore {
    conn: Connection,
}

impl MetricStore {
    /// Open (or create) a SQLite database at the given path.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create DB directory: {e}"))?;
        }

        let conn = Connection::open(path).map_err(|e| format!("Failed to open database: {e}"))?;

        conn.execute_batch(&format!(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; {SCHEMA_DDL}"
        ))
        .map_err(|e| format!("Failed to initialize schema: {e}"))?;

        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing).
    pub fn open_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory()
            .map_err(|e| format!("Failed to open in-memory database: {e}"))?;

        conn.execute_batch(SCHEMA_DDL)
            .map_err(|e| format!("Failed to initialize schema: {e}"))?;

        Ok(Self { conn })
    }

    pub fn write_snapshot(&self, snapshot: &SystemSnapshot) -> Result<(), String> {
        let json = serde_json::to_string(snapshot).map_err(|e| format!("Serialize error: {e}"))?;
        self.conn
            .execute(
                "INSERT INTO snapshots (timestamp, data) VALUES (?1, ?2)",
                params![snapshot.timestamp.to_rfc3339(), json],
            )
            .map_err(|e| format!("Insert error: {e}"))?;
        Ok(())
    }

    pub fn write_alerts(&self, alerts: &[Alert]) -> Result<(), String> {
        for a in alerts {
            self.conn
                .execute(
                    "INSERT INTO alerts (timestamp, severity, component, message) VALUES (?1, ?2, ?3, ?4)",
                    params![a.timestamp.to_rfc3339(), a.severity.to_string(), a.component, a.message],
                )
                .map_err(|e| format!("Insert alert error: {e}"))?;
        }
        Ok(())
    }

    pub fn write_predictions(&self, predictions: &[PredictionResult]) -> Result<(), String> {
        let json =
            serde_json::to_string(predictions).map_err(|e| format!("Serialize error: {e}"))?;
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO predictions (timestamp, data) VALUES (?1, ?2)",
                params![now, json],
            )
            .map_err(|e| format!("Insert error: {e}"))?;
        Ok(())
    }

    pub fn load_recent_snapshots(&self, n: usize) -> Result<Vec<SystemSnapshot>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM snapshots ORDER BY id DESC LIMIT ?1")
            .map_err(|e| format!("Query error: {e}"))?;

        let snapshots = stmt
            .query_map(params![n as i64], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })
            .map_err(|e| format!("Query error: {e}"))?
            .filter_map(|r| r.ok())
            .filter_map(|json| serde_json::from_str(&json).ok())
            .collect::<Vec<SystemSnapshot>>();

        // Reverse so oldest is first (for feeding into detector)
        Ok(snapshots.into_iter().rev().collect())
    }

    /// Delete rows older than `days` days. Returns number of deleted rows.
    pub fn prune_older_than(&self, days: u32) -> Result<usize, String> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();
        let mut total = 0usize;
        for table in &["snapshots", "alerts", "predictions"] {
            let deleted = self
                .conn
                .execute(
                    &format!("DELETE FROM {table} WHERE timestamp < ?1"),
                    params![cutoff_str],
                )
                .map_err(|e| format!("Prune error: {e}"))?;
            total += deleted;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nazar_core::*;
    use std::collections::HashMap;

    fn test_snapshot() -> SystemSnapshot {
        SystemSnapshot {
            timestamp: chrono::Utc::now(),
            cpu: CpuMetrics {
                cores: vec![50.0],
                total_percent: 50.0,
                load_average: [1.0, 1.0, 1.0],
                processes: 100,
                threads: 500,
            },
            memory: MemoryMetrics {
                total_bytes: 16_000_000_000,
                used_bytes: 8_000_000_000,
                available_bytes: 8_000_000_000,
                swap_total_bytes: 0,
                swap_used_bytes: 0,
                agent_usage: HashMap::new(),
            },
            disk: vec![],
            network: NetworkMetrics {
                interfaces: vec![],
                total_rx_bytes: 0,
                total_tx_bytes: 0,
                active_connections: 0,
            },
            temperatures: vec![],
            gpu: vec![],
            agents: AgentSummary::default(),
            services: vec![],
            top_processes: vec![],
        }
    }

    #[test]
    fn write_and_read_snapshot() {
        let store = MetricStore::open_memory().unwrap();
        let snap = test_snapshot();
        store.write_snapshot(&snap).unwrap();
        store.write_snapshot(&snap).unwrap();

        let loaded = store.load_recent_snapshots(10).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!((loaded[0].cpu.total_percent - 50.0).abs() < 0.01);
    }

    #[test]
    fn write_and_read_alerts() {
        let store = MetricStore::open_memory().unwrap();
        store
            .write_alerts(&[Alert {
                severity: AlertSeverity::Warning,
                component: "cpu".to_string(),
                message: "high".to_string(),
                timestamp: chrono::Utc::now(),
            }])
            .unwrap();
    }

    #[test]
    fn prune_deletes_old_rows() {
        let store = MetricStore::open_memory().unwrap();
        // Insert with old timestamp
        let old = chrono::Utc::now() - chrono::Duration::days(60);
        store
            .conn
            .execute(
                "INSERT INTO snapshots (timestamp, data) VALUES (?1, ?2)",
                params![old.to_rfc3339(), "{}"],
            )
            .unwrap();
        store.write_snapshot(&test_snapshot()).unwrap();

        let deleted = store.prune_older_than(30).unwrap();
        assert!(deleted >= 1);

        let remaining = store.load_recent_snapshots(100).unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn load_empty_db() {
        let store = MetricStore::open_memory().unwrap();
        let snaps = store.load_recent_snapshots(10).unwrap();
        assert!(snaps.is_empty());
    }
}
