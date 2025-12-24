use serde::{Deserialize, Serialize};

// ============================================================
// WebSocket Event Types
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "data")]
pub enum WSEvent {
    /// Strategy configuration (sent on connect)
    #[serde(rename = "config")]
    Config(serde_json::Value),

    /// Spot Grid strategy summary (high-level metrics)
    #[serde(rename = "spot_grid_summary")]
    SpotGridSummary(SpotGridSummary),

    /// Perp Grid strategy summary (high-level metrics)
    #[serde(rename = "perp_grid_summary")]
    PerpGridSummary(PerpGridSummary),

    /// Grid zone state for dashboard CLOB visualization
    #[serde(rename = "grid_state")]
    GridState(GridState),

    /// Order update (placed, filled, cancelled, failed)
    #[serde(rename = "order_update")]
    OrderUpdate(OrderEvent),

    /// Market price update
    #[serde(rename = "market_update")]
    MarketUpdate(MarketEvent),

    /// Error notification
    #[serde(rename = "error")]
    Error(String),
}

// ============================================================
// Strategy Summaries (High-level metrics)
// ============================================================

/// Spot Grid strategy summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotGridSummary {
    pub symbol: String,
    pub price: f64,
    pub state: String, // "Initializing", "Running", "AcquiringAssets", "WaitingForTrigger"
    pub uptime: String, // Human-readable uptime, e.g. "2d 14h 30m"

    // Position
    pub position_size: f64, // Base asset inventory
    pub avg_entry_price: f64,

    // PnL
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_fees: f64,
    pub start_price: Option<f64>,

    // Grid metrics
    pub grid_count: u32,
    pub range_low: f64,
    pub range_high: f64,
    pub grid_spacing_pct: (f64, f64), // (min%, max%) - same for geometric, different for arithmetic
    pub roundtrips: u32,              // Completed buy→sell cycles

    // Wallet balances
    pub base_balance: f64,
    pub quote_balance: f64,
}

/// Perp Grid strategy summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpGridSummary {
    pub symbol: String,
    pub price: f64,
    pub state: String,
    pub uptime: String, // Human-readable uptime, e.g. "2d 14h 30m"

    // Position
    pub position_size: f64,    // Positive = Long, Negative = Short
    pub position_side: String, // "Long", "Short", "Flat"
    pub avg_entry_price: f64,

    // PnL
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_fees: f64,

    // Grid/Perp specific
    pub leverage: u32,
    pub grid_bias: String, // "Long", "Short", "Neutral"
    pub grid_count: u32,
    pub range_low: f64,
    pub range_high: f64,
    pub grid_spacing_pct: (f64, f64), // (min%, max%) - same for geometric, different for arithmetic
    pub roundtrips: u32,

    // Wallet
    pub margin_balance: f64,
}

// ============================================================
// Grid State (Zone data for dashboard CLOB visualization)
// ============================================================

/// Grid zone state for dashboard visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridState {
    pub symbol: String,
    pub strategy_type: String, // "spot_grid" or "perp_grid"
    pub current_price: f64,
    pub grid_bias: Option<String>, // None for spot, "Long"/"Short"/"Neutral" for perp
    pub zones: Vec<ZoneInfo>,
}

/// Individual zone information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneInfo {
    pub index: usize,
    pub lower_price: f64,
    pub upper_price: f64,
    pub size: f64,

    // Raw order data
    pub pending_side: String, // "Buy" or "Sell"
    pub has_order: bool,
    pub is_reduce_only: bool, // For perp: closing orders are reduce_only

    // Metrics
    pub entry_price: f64,
    pub roundtrip_count: u32,
}

// ============================================================
// Order and Market Events (existing)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEvent {
    pub oid: u64,
    pub cloid: Option<String>,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub status: String,
    pub fee: f64,
    pub is_taker: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub price: f64,
}

// ============================================================
// Strategy Summary Enum (for trait return type)
// ============================================================

/// Wrapper enum for strategy-specific summaries
/// Used by the Strategy trait to return typed summaries
#[derive(Debug, Clone)]
pub enum StrategySummary {
    SpotGrid(SpotGridSummary),
    PerpGrid(PerpGridSummary),
}
