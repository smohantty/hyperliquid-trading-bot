use crate::model::OrderRequest;
use std::collections::HashMap;

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

    pub fn round_size(&self, size: f64) -> f64 {
        round_to_decimals(size, self.sz_decimals)
    }
}

pub struct StrategyContext {
    pub markets: HashMap<String, MarketInfo>,
    pub order_queue: Vec<OrderRequest>,
}

impl StrategyContext {
    pub fn new(markets: HashMap<String, MarketInfo>) -> Self {
        Self {
            markets,
            order_queue: Vec::new(),
        }
    }

    pub fn market_info(&self, symbol: &str) -> Option<&MarketInfo> {
        self.markets.get(symbol)
    }

    pub fn market_info_mut(&mut self, symbol: &str) -> Option<&mut MarketInfo> {
        self.markets.get_mut(symbol)
    }

    pub fn place_limit_order(
        &mut self,
        symbol: String,
        is_buy: bool,
        price: f64,
        sz: f64,
        reduce_only: bool,
    ) {
        self.order_queue.push(OrderRequest::Limit {
            symbol,
            is_buy,
            price,
            sz,
            reduce_only,
        });
    }

    pub fn place_market_order(&mut self, symbol: String, is_buy: bool, sz: f64) {
        self.order_queue
            .push(OrderRequest::Market { symbol, is_buy, sz });
    }
}
