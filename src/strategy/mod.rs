use crate::config::strategy::StrategyConfig;
use crate::error::BotError;

pub mod perp_grid;
pub mod spot_grid;

pub trait Strategy {
    fn run(&self) -> Result<(), BotError>;
}

pub fn init_strategy(config: StrategyConfig) -> Box<dyn Strategy> {
    match config {
        StrategyConfig::SpotGrid {
            symbol,
            upper_price,
            lower_price,
            grid_type,
            grid_count,
            total_investment,
            trigger_price,
        } => Box::new(spot_grid::SpotGridStrategy::new(
            symbol,
            upper_price,
            lower_price,
            grid_type,
            grid_count,
            total_investment,
            trigger_price,
        )),
        StrategyConfig::PerpGrid {
            symbol,
            leverage,
            is_isolated,
            grid_count,
            range_percent,
        } => Box::new(perp_grid::PerpGridStrategy::new(
            symbol,
            leverage,
            is_isolated,
            grid_count,
            range_percent,
        )),
    }
}
