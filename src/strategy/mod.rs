use crate::config::strategy::StrategyConfig;
// use crate::error::BotError;

pub mod perp_grid;
pub mod spot_grid;

use crate::engine::context::StrategyContext;
use anyhow::Result;

pub trait Strategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()>;
    // TODO: Add on_order_filled when we have the Order struct definition
}

pub fn init_strategy(config: StrategyConfig) -> Box<dyn Strategy> {
    match config {
        StrategyConfig::SpotGrid { .. } => Box::new(spot_grid::SpotGridStrategy::new(config)),
        StrategyConfig::PerpGrid { .. } => Box::new(perp_grid::PerpGridStrategy::new(config)),
    }
}
