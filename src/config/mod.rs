use self::bot::BotConfig;
use crate::error::BotError;
use std::fs;

pub mod bot;
pub mod broadcast;
pub mod creator;
pub mod exchange;
pub mod simulation;
pub mod strategy;

pub fn load_bot_config(path: &str) -> Result<BotConfig, BotError> {
    let content = fs::read_to_string(path)?;
    let config: BotConfig = toml::from_str(&content)?;
    config
        .validate()
        .map_err(|e| BotError::ValidationError(e.to_string()))?;
    Ok(config)
}
