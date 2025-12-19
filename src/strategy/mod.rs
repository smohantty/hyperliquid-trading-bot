use crate::config::strategy::StrategyConfig;
// use crate::error::BotError;

pub mod perp_grid;
pub mod spot_grid;

use crate::engine::context::StrategyContext;
use anyhow::Result;

pub trait Strategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()>;
    // Generic fill handling - using a minimal struct or SDK type?
    // Using simple arguments for now to match UserEvents parsing plan
    fn on_order_filled(
        &mut self,
        side: &str,
        size: f64,
        px: f64,
        cloid: Option<uuid::Uuid>,
        ctx: &mut StrategyContext,
    ) -> Result<()>;

    // State management
    fn save_state(&self) -> Result<String>;
    fn load_state(&mut self, state: &str) -> Result<()>;
}

pub fn init_strategy(config: StrategyConfig) -> Box<dyn Strategy> {
    match config {
        StrategyConfig::SpotGrid { .. } => Box::new(spot_grid::SpotGridStrategy::new(config)),
        StrategyConfig::PerpGrid { .. } => Box::new(perp_grid::PerpGridStrategy::new(config)),
    }
}
