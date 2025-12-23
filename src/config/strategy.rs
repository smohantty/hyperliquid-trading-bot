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
    false // Default to cross margin (more capital efficient for grid strategies)
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
                symbol,
                trigger_price,
                lower_price,
                upper_price,
                grid_count,
                total_investment,
                ..
            } => {
                // Common checks
                if *grid_count <= 2 {
                    return Err(anyhow::anyhow!(
                        "Grid count {} must be greater than 2.",
                        grid_count
                    ));
                }
                if *upper_price <= *lower_price {
                    return Err(anyhow::anyhow!(
                        "Upper price {} must be greater than lower price {}.",
                        upper_price,
                        lower_price
                    ));
                }
                if let Some(trigger) = trigger_price {
                    if *trigger < *lower_price || *trigger > *upper_price {
                        return Err(anyhow::anyhow!(
                            "Trigger price {} is outside the grid range [{}, {}].",
                            trigger,
                            lower_price,
                            upper_price
                        ));
                    }
                    if *trigger <= 0.0 {
                        return Err(anyhow::anyhow!("Trigger price must be positive."));
                    }
                }

                // Spot specific
                if !symbol.contains('/') || symbol.len() < 3 {
                    return Err(anyhow::anyhow!(
                        "Spot symbol must be in 'Base/Quote' format"
                    ));
                }
                if *total_investment <= 0.0 {
                    return Err(anyhow::anyhow!("Total investment must be positive."));
                }
            }
            StrategyConfig::PerpGrid {
                leverage,
                trigger_price,
                lower_price,
                upper_price,
                grid_count,
                total_investment,
                ..
            } => {
                // Common checks
                if *grid_count <= 2 {
                    return Err(anyhow::anyhow!(
                        "Grid count {} must be greater than 2.",
                        grid_count
                    ));
                }
                if *upper_price <= *lower_price {
                    return Err(anyhow::anyhow!(
                        "Upper price {} must be greater than lower price {}.",
                        upper_price,
                        lower_price
                    ));
                }
                if let Some(trigger) = trigger_price {
                    if *trigger < *lower_price || *trigger > *upper_price {
                        return Err(anyhow::anyhow!(
                            "Trigger price {} is outside the grid range [{}, {}].",
                            trigger,
                            lower_price,
                            upper_price
                        ));
                    }
                    if *trigger <= 0.0 {
                        return Err(anyhow::anyhow!("Trigger price must be positive."));
                    }
                }

                // Perp specific
                if *leverage == 0 || *leverage > 50 {
                    return Err(anyhow::anyhow!("Leverage must be between 1 and 50"));
                }
                if *total_investment <= 0.0 {
                    return Err(anyhow::anyhow!("Total investment must be positive."));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_upper_less_than_lower() {
        let config = StrategyConfig::SpotGrid {
            symbol: "BTC/USDC".to_string(),
            upper_price: 1000.0,
            lower_price: 2000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 10,
            total_investment: 1000.0,
            trigger_price: None,
        };
        let res = config.validate();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Upper price 1000 must be greater than lower price 2000."
        );
    }

    #[test]
    fn test_validation_trigger_out_of_bounds() {
        let config = StrategyConfig::SpotGrid {
            symbol: "BTC/USDC".to_string(),
            upper_price: 2000.0,
            lower_price: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 10,
            total_investment: 1000.0,
            trigger_price: Some(3000.0),
        };
        let res = config.validate();
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Trigger price"));
    }

    #[test]
    fn test_validation_grid_count_too_low() {
        let config = StrategyConfig::SpotGrid {
            symbol: "BTC/USDC".to_string(),
            upper_price: 2000.0,
            lower_price: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 2,
            total_investment: 1000.0,
            trigger_price: None,
        };
        let res = config.validate();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Grid count 2 must be greater than 2."
        );
    }

    #[test]
    fn test_validation_invalid_symbol_format() {
        let config = StrategyConfig::SpotGrid {
            symbol: "BTCUSDC".to_string(), // Missing '/'
            upper_price: 2000.0,
            lower_price: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            total_investment: 1000.0,
            trigger_price: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_negative_investment() {
        let config = StrategyConfig::SpotGrid {
            symbol: "BTC/USDC".to_string(),
            upper_price: 2000.0,
            lower_price: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            total_investment: -100.0,
            trigger_price: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_leverage() {
        // Zero leverage
        let config = StrategyConfig::PerpGrid {
            symbol: "BTC".to_string(),
            leverage: 0,
            is_isolated: true,
            upper_price: 2000.0,
            lower_price: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            total_investment: 1000.0,
            grid_bias: GridBias::Neutral,
            trigger_price: None,
        };
        assert!(config.validate().is_err());

        // Too high leverage
        let config2 = StrategyConfig::PerpGrid {
            symbol: "BTC".to_string(),
            leverage: 51,
            is_isolated: true,
            upper_price: 2000.0,
            lower_price: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            total_investment: 1000.0,
            grid_bias: GridBias::Neutral,
            trigger_price: None,
        };
        assert!(config2.validate().is_err());
    }

    #[test]
    fn test_validation_valid_configs() {
        let spot = StrategyConfig::SpotGrid {
            symbol: "BTC/USDC".to_string(),
            upper_price: 2000.0,
            lower_price: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 10,
            total_investment: 1000.0,
            trigger_price: None,
        };
        assert!(spot.validate().is_ok());

        let perp = StrategyConfig::PerpGrid {
            symbol: "BTC".to_string(),
            leverage: 10,
            is_isolated: true,
            upper_price: 2000.0,
            lower_price: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 10,
            total_investment: 1000.0,
            grid_bias: GridBias::Neutral,
            trigger_price: None,
        };
        assert!(perp.validate().is_ok());
    }
}
