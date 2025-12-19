use crate::config::strategy::StrategyConfig;
use crate::engine::context::StrategyContext;
use crate::strategy::Strategy;
use anyhow::Result;
use log::debug;

#[allow(dead_code)]
pub struct PerpGridStrategy {
    symbol: String,
    leverage: u32,
    is_isolated: bool,
    grid_count: u32,
    range_percent: f64,
}

impl PerpGridStrategy {
    pub fn new(config: StrategyConfig) -> Self {
        match config {
            StrategyConfig::PerpGrid {
                symbol,
                leverage,
                is_isolated,
                grid_count,
                range_percent,
            } => Self {
                symbol,
                leverage,
                is_isolated,
                grid_count,
                range_percent,
            },
            _ => panic!("Invalid config type for PerpGridStrategy"),
        }
    }
}

impl Strategy for PerpGridStrategy {
    fn on_tick(&mut self, price: f64, _ctx: &mut StrategyContext) -> Result<()> {
        debug!(
            "PerpGridStrategy received tick: Price = {}, Symbol = {}",
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
        _cloid: Option<uuid::Uuid>,
        _ctx: &mut StrategyContext,
    ) -> Result<()> {
        log::info!(
            "PerpGridStrategy: Order Filled - Side: {}, Size: {}, Price: {}",
            side,
            size,
            px
        );
        // Grid Logic to place next order would go here
        Ok(())
    }
}
