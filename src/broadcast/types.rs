use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "data")]
pub enum WSEvent {
    #[serde(rename = "config")]
    Config(serde_json::Value),
    #[serde(rename = "summary")]
    Summary(StatusSummary),
    #[serde(rename = "order_update")]
    OrderUpdate(OrderEvent),
    #[serde(rename = "market_update")]
    MarketUpdate(MarketEvent),
    #[serde(rename = "error")]
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSEnvelope {
    pub event_type: String,
    pub timestamp: i64,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusSummary {
    pub strategy_name: String,
    pub symbol: String,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_fees: f64,
    pub inventory: InventoryStats,
    pub wallet: WalletStats,
    pub price: f64,
    pub zones: Vec<ZoneStatus>,    // For visual order book
    pub custom: serde_json::Value, // Strategy-specific extras
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryStats {
    pub base_size: f64,
    pub avg_entry_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletStats {
    pub base_balance: f64,
    pub quote_balance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneStatus {
    pub price: f64,
    pub side: String,   // "Buy" or "Sell"
    pub status: String, // "Active", "Filled"
    pub size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEvent {
    pub oid: u64,
    pub cloid: Option<String>,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub status: String,
    pub fee: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub price: f64,
}
