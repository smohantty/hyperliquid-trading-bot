use serde::{Deserialize, Serialize};

pub use crate::strategy::types::{GridBias, GridType};

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
        #[serde(default = "default_is_isolated")]
        is_isolated: bool,
        upper_price: f64,
        lower_price: f64,
        grid_type: GridType,
        grid_count: u32,
        total_investment: f64,
        grid_bias: GridBias,
        #[serde(default)]
        trigger_price: Option<f64>,
    },
}

fn default_is_isolated() -> bool {
    true
}

impl StrategyConfig {
    pub fn type_name(&self) -> &str {
        match self {
            StrategyConfig::SpotGrid { .. } => "Spot Grid",
            StrategyConfig::PerpGrid { .. } => "Perp Grid",
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            StrategyConfig::SpotGrid { symbol, .. } => symbol,
            StrategyConfig::PerpGrid { symbol, .. } => symbol,
        }
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            StrategyConfig::SpotGrid {
                trigger_price,
                lower_price,
                upper_price,
                ..
            }
            | StrategyConfig::PerpGrid {
                trigger_price,
                lower_price,
                upper_price,
                ..
            } => {
                if let Some(trigger) = trigger_price {
                    if *trigger < *lower_price || *trigger > *upper_price {
                        return Err(anyhow::anyhow!(
                            "Trigger price {} is outside the grid range [{}, {}].",
                            trigger,
                            lower_price,
                            upper_price
                        ));
                    }
                }
            }
        }
        Ok(())
    }
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
    println!("     - is_isolated (bool): Isolated margin mode (default: true).");
    println!("     - upper_price (f64): The upper bound of the grid range.");
    println!("     - lower_price (f64): The lower bound of the grid range.");
    println!("     - grid_type (String): 'arithmetic' or 'geometric'.");
    println!("     - grid_count (u32): Number of grid levels.");
    println!(
        "     - total_investment (f64): Total cost basis in USDC.
     - grid_bias (String): 'long', 'short', or 'neutral'.
     - trigger_price (Option<f64>): Price to trigger strategy start (optional)."
    );
    println!();
}
