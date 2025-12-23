use self::strategy::StrategyConfig;
use crate::error::BotError;
use std::fs;

pub mod creator;
pub mod exchange;
pub mod strategy;

pub fn load_config(path: &str) -> Result<StrategyConfig, BotError> {
    let content = fs::read_to_string(path)?;
    let config: StrategyConfig = toml::from_str(&content)?;
    config
        .validate()
        .map_err(|e| BotError::ValidationError(e.to_string()))?;
    Ok(config)
}
