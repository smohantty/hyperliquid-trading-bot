use anyhow::{Context, Result};
use chrono::Local;
use csv::Writer;
use serde::Serialize;
use std::fs::{create_dir_all, OpenOptions};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Clone)]
pub struct OrderRecord {
    pub timestamp: String,
    pub symbol: String,
    pub order_type: String, // REQ, FILL
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub reduce_only: bool,
    pub cloid: Option<String>,
    pub fee: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Clone)]
pub struct OrderAuditLogger {
    writer: Arc<Mutex<Writer<std::fs::File>>>,
}

impl OrderAuditLogger {
    pub fn new(log_dir: &str) -> Result<Self> {
        let dir = Path::new(log_dir);
        create_dir_all(dir).context("Failed to create log directory")?;

        let file_path = dir.join("trades.csv");
        let file_exists = file_path.exists();

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .context("Failed to open trades.csv")?;

        let writer = csv::WriterBuilder::new()
            .has_headers(!file_exists)
            .from_writer(file);

        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
        })
    }

    pub fn log(&self, record: OrderRecord) {
        if let Ok(mut w) = self.writer.lock() {
            if let Err(e) = w.serialize(record) {
                eprintln!("Failed to write order audit log: {}", e);
            } else {
                let _ = w.flush();
            }
        }
    }

    pub fn log_req(
        &self,
        symbol: &str,
        side: &str,
        price: f64,
        size: f64,
        reduce_only: bool,
        cloid: Option<String>,
    ) {
        self.log(OrderRecord {
            timestamp: Local::now().to_rfc3339(),
            symbol: symbol.to_string(),
            order_type: "REQ".to_string(),
            side: side.to_string(),
            price,
            size,
            reduce_only,
            cloid,
            fee: None,
            notes: None,
        });
    }

    pub fn log_fill(
        &self,
        symbol: &str,
        side: &str,
        price: f64,
        size: f64,
        reduce_only: bool,
        cloid: Option<String>,
        fee: f64,
    ) {
        self.log(OrderRecord {
            timestamp: Local::now().to_rfc3339(),
            symbol: symbol.to_string(),
            order_type: "FILL".to_string(),
            side: side.to_string(),
            price,
            size,
            reduce_only,
            cloid,
            fee: Some(fee),
            notes: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_audit_log_header() {
        let dir = tempdir().unwrap();
        let log_dir = dir.path().to_str().unwrap();
        let logger = OrderAuditLogger::new(log_dir).unwrap();

        logger.log_req("BTC/USDC", "Buy", 50000.0, 1.0, false, None);

        let file_path = dir.path().join("trades.csv");
        let content = std::fs::read_to_string(file_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();

        // Should have exactly 2 lines: header + 1 record
        assert_eq!(lines.len(), 2);
        assert!(lines[0]
            .contains("timestamp,symbol,order_type,side,price,size,reduce_only,cloid,fee,notes"));
        assert!(lines[1].contains("BTC/USDC,REQ,Buy,50000.0,1.0,false"));
    }
}
