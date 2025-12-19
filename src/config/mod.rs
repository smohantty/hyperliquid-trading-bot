use self::strategy::StrategyConfig;
use crate::error::BotError;
use std::fs;

pub mod creator;
pub mod exchange;
pub mod strategy;

pub fn load_config(path: &str) -> Result<StrategyConfig, BotError> {
    let content = fs::read_to_string(path)?;
    let config: StrategyConfig = toml::from_str(&content)?;
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &StrategyConfig) -> Result<(), BotError> {
    match config {
        StrategyConfig::SpotGrid {
            symbol,
            upper_price,
            lower_price,
            grid_count,
            total_investment,
            trigger_price,
            ..
        } => {
            if !symbol.ends_with("/USDC") || symbol == "/USDC" {
                return Err(BotError::ValidationError(
                    "Spot symbol must be in 'Asset/USDC' format".into(),
                ));
            }
            if *grid_count == 0 {
                return Err(BotError::ValidationError("Grid count must be > 0".into()));
            }
            if upper_price <= lower_price {
                return Err(BotError::ValidationError(
                    "Upper price must be greater than lower price".into(),
                ));
            }
            if *total_investment <= 0.0 {
                return Err(BotError::ValidationError(
                    "Total investment must be > 0".into(),
                ));
            }
            if let Some(trigger) = trigger_price {
                if *trigger <= 0.0 {
                    return Err(BotError::ValidationError(
                        "Trigger price must be > 0".into(),
                    ));
                }
            }
        }
        StrategyConfig::PerpGrid {
            leverage,
            grid_count,
            ..
        } => {
            if *grid_count == 0 {
                return Err(BotError::ValidationError("Grid count must be > 0".into()));
            }
            if *leverage == 0 || *leverage > 50 {
                return Err(BotError::ValidationError(
                    "Leverage must be between 1 and 50".into(),
                ));
            }
        }
    }
    Ok(())
}
