use crate::error::BotError;
use crate::strategy::Strategy;

pub struct PerpGridStrategy {
    symbol: String,
    leverage: u32,
    is_isolated: bool,
    grid_count: u32,
    range_percent: f64,
}

impl PerpGridStrategy {
    pub fn new(
        symbol: String,
        leverage: u32,
        is_isolated: bool,
        grid_count: u32,
        range_percent: f64,
    ) -> Self {
        Self {
            symbol,
            leverage,
            is_isolated,
            grid_count,
            range_percent,
        }
    }
}

impl Strategy for PerpGridStrategy {
    fn run(&self) -> Result<(), BotError> {
        println!("Starting Perp Grid Strategy for {}", self.symbol);
        println!(
            "Leverage: {}x (Isolated: {})",
            self.leverage, self.is_isolated
        );
        println!("Grids: {}", self.grid_count);
        println!("Range: {}%", self.range_percent);
        Ok(())
    }
}
