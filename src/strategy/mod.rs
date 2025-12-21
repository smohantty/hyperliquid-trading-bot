use crate::config::strategy::StrategyConfig;
// use crate::error::BotError;

pub mod common;
pub mod perp_grid;
pub mod spot_grid;
pub mod types;

use crate::engine::context::StrategyContext;
use crate::model::{Cloid, OrderFill};
use anyhow::Result;

pub trait Strategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()>;
    fn on_order_filled(&mut self, fill: &OrderFill, ctx: &mut StrategyContext) -> Result<()>;

    fn on_order_failed(&mut self, cloid: Cloid, ctx: &mut StrategyContext) -> Result<()>;

    fn get_status_snapshot(&self, ctx: &StrategyContext) -> crate::broadcast::types::StatusSummary;
}

pub fn init_strategy(config: StrategyConfig) -> Result<Box<dyn Strategy>> {
    config.validate()?;
    match config {
        StrategyConfig::SpotGrid { .. } => Ok(Box::new(spot_grid::SpotGridStrategy::new(config))),
        StrategyConfig::PerpGrid { .. } => Ok(Box::new(perp_grid::PerpGridStrategy::new(config))),
    }
}
