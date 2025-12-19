use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum GridType {
    Arithmetic,
    Geometric,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum StrategyConfig {
    #[serde(rename = "spot_grid")]
    SpotGrid {
        symbol: String,
        upper_price: f64,
        lower_price: f64,
        grid_type: GridType,
        grid_count: u32,
        total_investment: f64,
        #[serde(default)]
        trigger_price: Option<f64>,
    },
    #[serde(rename = "perp_grid")]
    PerpGrid {
        symbol: String,
        leverage: u32,
        is_isolated: bool,
        grid_count: u32,
        range_percent: f64,
    },
}

pub fn print_strategy_help() {
    println!("Available Strategies:\n");

    println!("1. Spot Grid Strategy (type = 'spot_grid')");
    println!("   Description: A grid trading strategy for spot markets.");
    println!("   Parameters:");
    println!("     - symbol (String): The trading pair symbol (e.g., 'ETH/USDC').");
    println!("     - upper_price (f64): The upper bound of the grid range.");
    println!("     - lower_price (f64): The lower bound of the grid range.");
    println!("     - grid_type (String): 'arithmetic' or 'geometric'.");
    println!("     - grid_count (u32): Number of grid levels.");
    println!("     - total_investment (f64): Total base asset value to invest.");
    println!("     - trigger_price (Option<f64>): Price to trigger strategy start (optional).");
    println!();

    println!("2. Perp Grid Strategy (type = 'perp_grid')");
    println!("   Description: A grid trading strategy for perpetual futures.");
    println!("   Parameters:");
    println!("     - symbol (String): The trading pair symbol (e.g., 'BTC').");
    println!("     - leverage (u32): Leverage multiplier (1-50x).");
    println!("     - is_isolated (bool): Isolated margin mode.");
    println!("     - grid_count (u32): Number of grid levels.");
    println!("     - range_percent (f64): Price range percentage.");
    println!();
}
