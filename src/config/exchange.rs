use crate::config::read_env_or_file;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub private_key: String,
    pub network: String,
}

pub fn load_exchange_config() -> Result<ExchangeConfig> {
    // Load .env file if it exists, ignore if missing (env vars might be set otherwise)
    // Note: dotenvy::dotenv is no longer directly used here, but the comment remains for context.
    // The read_env_or_file function might handle its own dotenv loading or expect it to be done.
    // Assuming dotenvy::dotenv() is still desired for general env loading, it's kept.
    // If read_env_or_file handles dotenv loading internally, this line could be removed.
    // For now, keeping it as it was in the original context.
    dotenvy::dotenv().ok();

    let private_key = read_env_or_file("HYPERLIQUID_PRIVATE_KEY")
        .context("HYPERLIQUID_PRIVATE_KEY must be set")?;

    let network = env::var("HYPERLIQUID_NETWORK").unwrap_or_else(|_| "mainnet".to_string());

    Ok(ExchangeConfig {
        private_key,
        network,
    })
}
