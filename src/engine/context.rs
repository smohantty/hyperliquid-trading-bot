use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MarketInfo {
    pub symbol: String,
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
    pub fn new(symbol: String, sz_decimals: u32, price_decimals: u32) -> Self {
        Self {
            symbol,
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
    // In the future, this will hold the ExchangeClient or a channel to send orders to the Engine
    // For now, we keep it simple.
    // We might need to pass the Engine's command sender here.
}

impl StrategyContext {
    pub fn new(markets: HashMap<String, MarketInfo>) -> Self {
        Self { markets }
    }

    pub fn market_info(&self, symbol: &str) -> Option<&MarketInfo> {
        self.markets.get(symbol)
    }

    pub fn market_info_mut(&mut self, symbol: &str) -> Option<&mut MarketInfo> {
        self.markets.get_mut(symbol)
    }

    // Placeholder for order placement
    // pub fn place_order(...) {}
}
