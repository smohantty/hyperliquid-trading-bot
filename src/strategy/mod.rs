use crate::config::strategy::StrategyConfig;

pub mod common;
pub mod perp_grid;
pub mod spot_grid;
pub mod types;

use crate::broadcast::types::{GridState, StrategySummary};
use crate::engine::context::StrategyContext;
use crate::model::{Cloid, OrderFill};
use anyhow::Result;

/// Core strategy trait that all trading strategies must implement
pub trait Strategy {
    /// Called on each price tick
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()>;

    /// Called when an order is filled
    fn on_order_filled(&mut self, fill: &OrderFill, ctx: &mut StrategyContext) -> Result<()>;

    /// Called when an order fails
    fn on_order_failed(&mut self, cloid: Cloid, ctx: &mut StrategyContext) -> Result<()>;

    /// Returns high-level strategy metrics for summary display
    /// Called periodically by the engine (e.g., every 1-2 seconds)
    fn get_summary(&self, ctx: &StrategyContext) -> StrategySummary;

    /// Returns grid zone state for dashboard visualization
    /// Called after order fills when grid state changes
    fn get_grid_state(&self, ctx: &StrategyContext) -> GridState;
}

/// Initialize a strategy from configuration
pub fn init_strategy(config: StrategyConfig) -> Result<Box<dyn Strategy>> {
    config.validate()?;
    match config {
        StrategyConfig::SpotGrid(c) => Ok(Box::new(spot_grid::SpotGridStrategy::new(c))),
        StrategyConfig::PerpGrid(c) => Ok(Box::new(perp_grid::PerpGridStrategy::new(c))),
    }
}
