use self::strategy::StrategyConfig;
use crate::error::BotError;
use std::env;
use std::fs;

pub mod broadcast;
pub mod creator;
pub mod exchange;
pub mod strategy;

#[cfg(test)]
mod test_secrets;

pub fn load_config(path: &str) -> Result<StrategyConfig, BotError> {
    let content = fs::read_to_string(path)?;
    let config: StrategyConfig = toml::from_str(&content)?;
    config
        .validate()
        .map_err(|e| BotError::ValidationError(e.to_string()))?;
    Ok(config)
}

pub fn read_env_or_file(key: &str) -> anyhow::Result<String> {
    // 1. Try checking the env var directly
    if let Ok(val) = env::var(key) {
        return Ok(val);
    }

    // 2. Try checking key_FILE
    let file_env_key = format!("{}_FILE", key);
    if let Ok(path) = env::var(&file_env_key) {
        let content = fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read secret file {}: {}", path, e))?;
        return Ok(content.trim().to_string());
    }

    Err(anyhow::anyhow!(
        "Environment variable {} (or {}) not found",
        key,
        file_env_key
    ))
}
