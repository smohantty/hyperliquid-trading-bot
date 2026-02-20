use serde::{Deserialize, Serialize};

pub use crate::strategy::types::{GridBias, GridType};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum StrategyConfig {
    #[serde(rename = "spot_grid")]
    SpotGrid(SpotGridConfig),
    #[serde(rename = "perp_grid")]
    PerpGrid(PerpGridConfig),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SpotGridConfig {
    pub symbol: String,
    pub grid_range_high: f64,
    pub grid_range_low: f64,
    pub grid_type: GridType,
    /// Number of grid levels. Either grid_count OR spread_bips must be provided.
    #[serde(default)]
    pub grid_count: Option<u32>,
    /// Spread in basis points between levels. Either grid_count OR spread_bips must be provided.
    #[serde(default)]
    pub spread_bips: Option<f64>,
    pub total_investment: f64,
    #[serde(default)]
    pub trigger_price: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PerpGridConfig {
    pub symbol: String,
    pub leverage: u32,
    #[serde(default = "default_is_isolated")]
    pub is_isolated: bool,
    pub grid_range_high: f64,
    pub grid_range_low: f64,
    pub grid_type: GridType,
    pub grid_count: u32,
    #[serde(default)]
    pub spread_bips: Option<f64>,
    pub total_investment: f64,
    pub grid_bias: GridBias,
    #[serde(default)]
    pub trigger_price: Option<f64>,
}

fn default_is_isolated() -> bool {
    false // Default to cross margin (more capital efficient for grid strategies)
}

impl StrategyConfig {
    pub fn type_name(&self) -> &str {
        match self {
            StrategyConfig::SpotGrid(_) => "Spot Grid",
            StrategyConfig::PerpGrid(_) => "Perp Grid",
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            StrategyConfig::SpotGrid(c) => &c.symbol,
            StrategyConfig::PerpGrid(c) => &c.symbol,
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            StrategyConfig::SpotGrid(c) => c.validate(),
            StrategyConfig::PerpGrid(c) => c.validate(),
        }
    }
}

impl SpotGridConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        // Must have either grid_count OR spread_bips, not both
        match (self.grid_count, self.spread_bips) {
            (None, None) => {
                return Err(anyhow::anyhow!(
                    "Either grid_count or spread_bips must be specified."
                ));
            }
            (Some(_), Some(_)) => {
                return Err(anyhow::anyhow!(
                    "Only one of grid_count or spread_bips can be specified, not both."
                ));
            }
            _ => {}
        }

        // Validate grid_count if present
        if let Some(count) = self.grid_count {
            if count <= 2 {
                return Err(anyhow::anyhow!(
                    "Grid count {} must be greater than 2.",
                    count
                ));
            }
        }

        // Validate spread_bips if present
        if let Some(bips) = self.spread_bips {
            if bips <= 0.0 {
                return Err(anyhow::anyhow!("spread_bips {} must be positive.", bips));
            }
        }

        if self.grid_range_high <= self.grid_range_low {
            return Err(anyhow::anyhow!(
                "Upper price {} must be greater than lower price {}.",
                self.grid_range_high,
                self.grid_range_low
            ));
        }
        if let Some(trigger) = self.trigger_price {
            if trigger < self.grid_range_low || trigger > self.grid_range_high {
                return Err(anyhow::anyhow!(
                    "Trigger price {} is outside the grid range [{}, {}].",
                    trigger,
                    self.grid_range_low,
                    self.grid_range_high
                ));
            }
            if trigger <= 0.0 {
                return Err(anyhow::anyhow!("Trigger price must be positive."));
            }
        }

        // Spot specific
        if !self.symbol.contains('/') || self.symbol.len() < 3 {
            return Err(anyhow::anyhow!(
                "Spot symbol must be in 'Base/Quote' format"
            ));
        }
        if self.total_investment <= 0.0 {
            return Err(anyhow::anyhow!("Total investment must be positive."));
        }
        Ok(())
    }

    /// Get the effective grid count, either from config or calculated from spread_bips.
    /// This should only be called after zones have been generated.
    pub fn get_grid_count(&self) -> u32 {
        self.grid_count.unwrap_or(0)
    }
}

impl PerpGridConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        // Common checks
        if self.grid_count <= 2 {
            return Err(anyhow::anyhow!(
                "Grid count {} must be greater than 2.",
                self.grid_count
            ));
        }
        if self.grid_range_high <= self.grid_range_low {
            return Err(anyhow::anyhow!(
                "Upper price {} must be greater than lower price {}.",
                self.grid_range_high,
                self.grid_range_low
            ));
        }
        if let Some(trigger) = self.trigger_price {
            if trigger < self.grid_range_low || trigger > self.grid_range_high {
                return Err(anyhow::anyhow!(
                    "Trigger price {} is outside the grid range [{}, {}].",
                    trigger,
                    self.grid_range_low,
                    self.grid_range_high
                ));
            }
            if trigger <= 0.0 {
                return Err(anyhow::anyhow!("Trigger price must be positive."));
            }
        }

        // Perp specific
        if self.leverage == 0 || self.leverage > 50 {
            return Err(anyhow::anyhow!("Leverage must be between 1 and 50"));
        }
        if self.total_investment <= 0.0 {
            return Err(anyhow::anyhow!("Total investment must be positive."));
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
    println!("     - grid_range_high (f64): The upper bound of the grid range.");
    println!("     - grid_range_low (f64): The lower bound of the grid range.");
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
    println!("     - grid_range_high (f64): The upper bound of the grid range.");
    println!("     - grid_range_low (f64): The lower bound of the grid range.");
    println!("     - grid_type (String): 'arithmetic' or 'geometric'.");
    println!("     - grid_count (u32): Number of grid levels.");
    println!(
        "     - total_investment (f64): Total cost basis in USDC.
     - grid_bias (String): 'long' or 'short'.
     - trigger_price (Option<f64>): Price to trigger strategy start (optional)."
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_upper_less_than_lower() {
        let config = StrategyConfig::SpotGrid(SpotGridConfig {
            symbol: "BTC/USDC".to_string(),
            grid_range_high: 1000.0,
            grid_range_low: 2000.0,
            grid_type: GridType::Arithmetic,
            grid_count: Some(10),
            spread_bips: None,
            total_investment: 1000.0,
            trigger_price: None,
        });
        let res = config.validate();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Upper price 1000 must be greater than lower price 2000."
        );
    }

    #[test]
    fn test_validation_trigger_out_of_bounds() {
        let config = StrategyConfig::SpotGrid(SpotGridConfig {
            symbol: "BTC/USDC".to_string(),
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: Some(10),
            spread_bips: None,
            total_investment: 1000.0,
            trigger_price: Some(3000.0),
        });
        let res = config.validate();
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Trigger price"));
    }

    #[test]
    fn test_validation_grid_count_too_low() {
        let config = StrategyConfig::SpotGrid(SpotGridConfig {
            symbol: "BTC/USDC".to_string(),
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: Some(2),
            spread_bips: None,
            total_investment: 1000.0,
            trigger_price: None,
        });
        let res = config.validate();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Grid count 2 must be greater than 2."
        );
    }

    #[test]
    fn test_validation_invalid_symbol_format() {
        let config = StrategyConfig::SpotGrid(SpotGridConfig {
            symbol: "BTCUSDC".to_string(), // Missing '/'
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: Some(5),
            spread_bips: None,
            total_investment: 1000.0,
            trigger_price: None,
        });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_negative_investment() {
        let config = StrategyConfig::SpotGrid(SpotGridConfig {
            symbol: "BTC/USDC".to_string(),
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: Some(5),
            spread_bips: None,
            total_investment: -100.0,
            trigger_price: None,
        });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_leverage() {
        // Zero leverage
        let config = StrategyConfig::PerpGrid(PerpGridConfig {
            symbol: "BTC".to_string(),
            leverage: 0,
            is_isolated: true,
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            spread_bips: None,
            total_investment: 1000.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        });
        assert!(config.validate().is_err());

        // Too high leverage
        let config2 = StrategyConfig::PerpGrid(PerpGridConfig {
            symbol: "BTC".to_string(),
            leverage: 51,
            is_isolated: true,
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            spread_bips: None,
            total_investment: 1000.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        });
        assert!(config2.validate().is_err());
    }

    #[test]
    fn test_validation_valid_configs() {
        let spot = StrategyConfig::SpotGrid(SpotGridConfig {
            symbol: "BTC/USDC".to_string(),
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: Some(10),
            spread_bips: None,
            total_investment: 1000.0,
            trigger_price: None,
        });
        assert!(spot.validate().is_ok());

        // Test with spread_bips instead of grid_count
        let spot_bips = StrategyConfig::SpotGrid(SpotGridConfig {
            symbol: "ETH/USDC".to_string(),
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Geometric,
            grid_count: None,
            spread_bips: Some(100.0), // 1%
            total_investment: 1000.0,
            trigger_price: None,
        });
        assert!(spot_bips.validate().is_ok());

        let perp = StrategyConfig::PerpGrid(PerpGridConfig {
            symbol: "BTC".to_string(),
            leverage: 10,
            is_isolated: true,
            grid_range_high: 2000.0,
            grid_range_low: 1000.0,
            grid_type: GridType::Arithmetic,
            grid_count: 10,
            spread_bips: None,
            total_investment: 1000.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        });
        assert!(perp.validate().is_ok());
    }
}
