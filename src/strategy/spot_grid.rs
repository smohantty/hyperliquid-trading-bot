use crate::error::BotError;
use crate::strategy::Strategy;

use crate::config::strategy::GridType;

pub struct SpotGridStrategy {
    symbol: String,
    upper_price: f64,
    lower_price: f64,
    grid_type: GridType,
    grid_count: u32,
    total_investment: f64,
    trigger_price: Option<f64>,
}

impl SpotGridStrategy {
    pub fn new(
        symbol: String,
        upper_price: f64,
        lower_price: f64,
        grid_type: GridType,
        grid_count: u32,
        total_investment: f64,
        trigger_price: Option<f64>,
    ) -> Self {
        Self {
            symbol,
            upper_price,
            lower_price,
            grid_type,
            grid_count,
            total_investment,
            trigger_price,
        }
    }
}

impl Strategy for SpotGridStrategy {
    fn run(&self) -> Result<(), BotError> {
        println!("Starting Spot Grid Strategy for {}", self.symbol);
        println!("Range: {} - {}", self.lower_price, self.upper_price);
        println!("Grid Type: {:?}", self.grid_type);
        println!("Grids: {}", self.grid_count);
        println!("Total Investment: {}", self.total_investment);
        if let Some(price) = self.trigger_price {
            println!("Trigger Price: {}", price);
        }
        Ok(())
    }
}
