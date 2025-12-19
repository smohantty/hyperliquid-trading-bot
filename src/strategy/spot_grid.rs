use crate::config::strategy::{GridType, StrategyConfig};
use crate::engine::context::StrategyContext;
use crate::strategy::Strategy;
use anyhow::Result;
use log::debug;

#[allow(dead_code)]
pub struct SpotGridStrategy {
    symbol: String,
    upper_price: f64,
    lower_price: f64,
    grid_type: GridType,
    grid_count: u32,
    total_investment: f64,
    #[allow(dead_code)]
    trigger_price: Option<f64>,
}

impl SpotGridStrategy {
    pub fn new(config: StrategyConfig) -> Self {
        match config {
            StrategyConfig::SpotGrid {
                symbol,
                upper_price,
                lower_price,
                grid_type,
                grid_count,
                total_investment,
                trigger_price,
            } => Self {
                symbol,
                upper_price,
                lower_price,
                grid_type,
                grid_count,
                total_investment,
                trigger_price,
            },
            _ => panic!("Invalid config type for SpotGridStrategy"),
        }
    }
}

impl Strategy for SpotGridStrategy {
    fn on_tick(&mut self, price: f64, _ctx: &mut StrategyContext) -> Result<()> {
        debug!(
            "SpotGridStrategy received tick: Price = {}, Symbol = {}",
            price, self.symbol
        );
        // Placeholder for grid logic
        Ok(())
    }
    fn on_order_filled(
        &mut self,
        side: &str,
        size: f64,
        px: f64,
        _ctx: &mut StrategyContext,
    ) -> Result<()> {
        log::info!(
            "SpotGridStrategy: Order Filled - Side: {}, Size: {}, Price: {}",
            side,
            size,
            px
        );
        // Grid Logic to place next order would go here
        Ok(())
    }
}
