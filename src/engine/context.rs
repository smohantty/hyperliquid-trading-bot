use crate::model::OrderRequest;
use std::collections::HashMap;

pub const MIN_NOTIONAL_VALUE: f64 = 11.0;

#[derive(Debug, Clone)]
pub struct MarketInfo {
    pub symbol: String,
    pub coin: String, // API identifier
    pub asset_index: u32,
    pub sz_decimals: u32,
    pub price_decimals: u32,
    pub last_price: f64,
}

fn round_to_decimals(value: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (value * factor).round() / factor
}

fn round_to_significant_and_decimal(value: f64, sig_figs: u32, max_decimals: u32) -> f64 {
    if value.abs() < 1e-9 {
        return 0.0;
    }
    let abs_value = value.abs();
    let magnitude = abs_value.log10().floor() as i32;
    let scale = 10f64.powi(sig_figs as i32 - magnitude - 1);
    let rounded = (abs_value * scale).round() / scale;
    round_to_decimals(rounded.copysign(value), max_decimals)
}

impl MarketInfo {
    pub fn new(
        symbol: String,
        coin: String,
        asset_index: u32,
        sz_decimals: u32,
        price_decimals: u32,
    ) -> Self {
        Self {
            symbol,
            coin,
            asset_index,
            sz_decimals,
            price_decimals,
            last_price: 0.0,
        }
    }

    pub fn round_price(&self, price: f64) -> f64 {
        // Hyperliquid uses 5 significant figures
        round_to_significant_and_decimal(price, 5, self.price_decimals)
    }

    pub fn round_size(&self, sz: f64) -> f64 {
        round_to_decimals(sz, self.sz_decimals)
    }

    /// Clamps the given size to the minimum required to meet the notional USD value.
    /// Returns the provided size rounded to asset precision, or the minimum required size if the notional is too low.
    pub fn clamp_to_min_notional(&self, size: f64, price: f64, min_notional: f64) -> f64 {
        let rounded_size = self.round_size(size);
        if price > 0.0 && (rounded_size * price < min_notional) {
            self.round_size(min_notional / price)
        } else {
            rounded_size
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Balance {
    pub total: f64,
    pub available: f64,
}

pub struct StrategyContext {
    pub markets: HashMap<String, MarketInfo>,
    pub spot_balances: HashMap<String, Balance>,
    pub perp_balances: HashMap<String, Balance>,
    pub order_queue: Vec<OrderRequest>,
    pub cancellation_queue: Vec<u128>,
    pub next_cloid: u128,
}

impl StrategyContext {
    pub fn new(markets: HashMap<String, MarketInfo>) -> Self {
        Self {
            markets,
            spot_balances: HashMap::new(),
            perp_balances: HashMap::new(),
            order_queue: Vec::new(),
            cancellation_queue: Vec::new(),
            next_cloid: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        }
    }

    pub fn market_info(&self, symbol: &str) -> Option<&MarketInfo> {
        self.markets.get(symbol)
    }

    pub fn market_info_mut(&mut self, symbol: &str) -> Option<&mut MarketInfo> {
        self.markets.get_mut(symbol)
    }

    pub fn place_order(&mut self, order: OrderRequest) {
        self.order_queue.push(order);
    }

    pub fn cancel_order(&mut self, cloid: u128) {
        self.cancellation_queue.push(cloid);
    }

    pub fn generate_cloid(&mut self) -> u128 {
        let cloid = self.next_cloid;
        self.next_cloid += 1;
        cloid
    }

    // --- Balance Accessors ---

    pub fn update_spot_balance(&mut self, asset: String, total: f64, available: f64) {
        self.spot_balances
            .insert(asset, Balance { total, available });
    }

    pub fn update_perp_balance(&mut self, asset: String, total: f64, available: f64) {
        self.perp_balances
            .insert(asset, Balance { total, available });
    }

    pub fn get_spot_total(&self, asset: &str) -> f64 {
        self.spot_balances
            .get(asset)
            .map(|b| b.total)
            .unwrap_or(0.0)
    }

    pub fn get_spot_available(&self, asset: &str) -> f64 {
        self.spot_balances
            .get(asset)
            .map(|b| b.available)
            .unwrap_or(0.0)
    }

    pub fn get_perp_total(&self, asset: &str) -> f64 {
        self.perp_balances
            .get(asset)
            .map(|b| b.total)
            .unwrap_or(0.0)
    }

    pub fn get_perp_available(&self, asset: &str) -> f64 {
        self.perp_balances
            .get(asset)
            .map(|b| b.available)
            .unwrap_or(0.0)
    }
}
