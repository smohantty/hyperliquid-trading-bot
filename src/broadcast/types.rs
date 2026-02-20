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

    /// System information (network, etc.)
    #[serde(rename = "info")]
    Info(SystemInfo),

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

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub network: String,
    pub exchange: String,
}

// ============================================================
// Strategy Summaries (High-level metrics)
// ============================================================

/// Spot Grid strategy summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotGridSummary {
    pub symbol: String,
    pub state: String, // "Initializing", "Running", "AcquiringAssets", "WaitingForTrigger"
    pub uptime: String, // Human-readable uptime, e.g. "2d 14h 30m"

    // Position
    pub position_size: f64, // Base asset inventory

    // PnL (aligned with Python)
    pub matched_profit: f64, // Profit from completed roundtrips
    pub total_profit: f64,   // current_equity - initial_equity - fees
    pub total_fees: f64,
    pub initial_entry_price: Option<f64>,

    // Grid metrics
    pub grid_count: u32,
    pub grid_range_low: f64,
    pub grid_range_high: f64,
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
    pub state: String,
    pub uptime: String, // Human-readable uptime, e.g. "2d 14h 30m"

    // Position
    pub position_size: f64,    // Positive = Long, Negative = Short
    pub position_side: String, // "Long", "Short", "Flat"
    pub avg_entry_price: f64,

    // PnL
    pub matched_profit: f64,
    pub total_profit: f64,
    pub unrealized_pnl: f64,
    pub total_fees: f64,

    // Grid/Perp specific
    pub leverage: u32,
    pub grid_bias: String, // "long", "short", "neutral"
    pub grid_count: u32,
    pub grid_range_low: f64,
    pub grid_range_high: f64,
    pub grid_spacing_pct: (f64, f64), // (min%, max%) - same for geometric, different for arithmetic
    pub roundtrips: u32,

    // Wallet
    pub margin_balance: f64,
    pub initial_entry_price: Option<f64>,
}

// ============================================================
// Grid State (Zone data for dashboard CLOB visualization)
// ============================================================

/// Grid zone state for dashboard visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridState {
    pub symbol: String,
    pub strategy_type: String,     // "spot_grid" or "perp_grid"
    pub grid_bias: Option<String>, // None for spot, "Long"/"Short"/"Neutral" for perp
    pub zones: Vec<ZoneInfo>,
}

/// Individual zone information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneInfo {
    pub index: usize,
    pub buy_price: f64,
    pub sell_price: f64,
    pub size: f64,

    // Raw order data
    pub order_side: String, // "Buy" or "Sell"
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

#[cfg(test)]
mod schema_tests {
    use super::*;

    fn load_schema() -> serde_json::Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/bot-ws-schema/schema/events.json"
        );
        let content = std::fs::read_to_string(path).expect("schema file not found");
        serde_json::from_str(&content).expect("invalid json in schema")
    }

    fn validate_event(event: &WSEvent) {
        let schema_json = load_schema();
        let validator = jsonschema::validator_for(&schema_json).expect("invalid schema");
        let event_json = serde_json::to_value(event).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&event_json)
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect();
        if !errors.is_empty() {
            panic!(
                "Schema validation failed for {:?}:\n{}",
                event_json,
                errors.join("\n")
            );
        }
    }

    #[test]
    fn test_config_spot_event() {
        let config = serde_json::json!({
            "type": "spot_grid",
            "symbol": "ETH/USDC",
            "grid_range_high": 4000.0,
            "grid_range_low": 3000.0,
            "grid_type": "arithmetic",
            "grid_count": 10,
            "total_investment": 1000.0
        });
        let event = WSEvent::Config(config);
        validate_event(&event);
    }

    #[test]
    fn test_config_perp_event() {
        let config = serde_json::json!({
            "type": "perp_grid",
            "symbol": "HYPE",
            "leverage": 5,
            "grid_range_high": 30.0,
            "grid_range_low": 20.0,
            "grid_type": "geometric",
            "grid_count": 20,
            "total_investment": 5000.0,
            "grid_bias": "long"
        });
        let event = WSEvent::Config(config);
        validate_event(&event);
    }

    #[test]
    fn test_info_event() {
        let event = WSEvent::Info(SystemInfo {
            network: "mainnet".to_string(),
            exchange: "hyperliquid".to_string(),
        });
        validate_event(&event);
    }

    #[test]
    fn test_spot_grid_summary_event() {
        let event = WSEvent::SpotGridSummary(SpotGridSummary {
            symbol: "ETH/USDC".to_string(),
            state: "Running".to_string(),
            uptime: "2d 14h 30m".to_string(),
            position_size: 1.5,
            matched_profit: 45.23,
            total_profit: 52.10,
            total_fees: 3.12,
            initial_entry_price: Some(3500.0),
            grid_count: 10,
            grid_range_low: 3000.0,
            grid_range_high: 4000.0,
            grid_spacing_pct: (1.05, 1.05),
            roundtrips: 12,
            base_balance: 1.5,
            quote_balance: 500.0,
        });
        validate_event(&event);
    }

    #[test]
    fn test_perp_grid_summary_event() {
        let event = WSEvent::PerpGridSummary(PerpGridSummary {
            symbol: "HYPE".to_string(),
            state: "Running".to_string(),
            uptime: "1d 8h 15m".to_string(),
            position_size: 100.0,
            position_side: "Long".to_string(),
            avg_entry_price: 24.8,
            matched_profit: 120.50,
            total_profit: 135.20,
            unrealized_pnl: 23.0,
            total_fees: 8.30,
            leverage: 5,
            grid_bias: "long".to_string(),
            grid_count: 20,
            grid_range_low: 20.0,
            grid_range_high: 30.0,
            grid_spacing_pct: (0.5, 0.5),
            roundtrips: 8,
            margin_balance: 1135.20,
            initial_entry_price: Some(25.0),
        });
        validate_event(&event);
    }

    #[test]
    fn test_grid_state_event() {
        let event = WSEvent::GridState(GridState {
            symbol: "HYPE".to_string(),
            strategy_type: "perp_grid".to_string(),
            grid_bias: Some("long".to_string()),
            zones: vec![
                ZoneInfo {
                    index: 0,
                    buy_price: 20.0,
                    sell_price: 20.5,
                    size: 10.0,
                    order_side: "Buy".to_string(),
                    has_order: true,
                    is_reduce_only: false,
                    entry_price: 20.25,
                    roundtrip_count: 1,
                },
                ZoneInfo {
                    index: 1,
                    buy_price: 20.5,
                    sell_price: 21.0,
                    size: 10.0,
                    order_side: "Sell".to_string(),
                    has_order: true,
                    is_reduce_only: true,
                    entry_price: 20.75,
                    roundtrip_count: 0,
                },
            ],
        });
        validate_event(&event);
    }

    #[test]
    fn test_order_update_event() {
        let event = WSEvent::OrderUpdate(OrderEvent {
            oid: 123456,
            cloid: Some("0xa1b2c3d4".to_string()),
            side: "Buy".to_string(),
            price: 3500.0,
            size: 0.1,
            status: "filled".to_string(),
            fee: 0.035,
            is_taker: false,
        });
        validate_event(&event);
    }

    #[test]
    fn test_market_update_event() {
        let event = WSEvent::MarketUpdate(MarketEvent { price: 3542.75 });
        validate_event(&event);
    }

    #[test]
    fn test_error_event() {
        let event = WSEvent::Error("Connection lost".to_string());
        validate_event(&event);
    }
}
