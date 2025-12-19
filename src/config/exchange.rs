use anyhow::{Context, Result};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub private_key: String,
    pub api_key: Option<String>,
    pub network: String,
}

pub fn load_exchange_config() -> Result<ExchangeConfig> {
    dotenv().ok(); // Load .env file if it exists, ignore if missing (env vars might be set otherwise)

    let private_key =
        env::var("HYPERLIQUID_PRIVATE_KEY").context("HYPERLIQUID_PRIVATE_KEY is not set")?;

    let api_key = env::var("HYPERLIQUID_API_KEY").ok();

    let network = env::var("HYPERLIQUID_NETWORK").unwrap_or_else(|_| "mainnet".to_string());

    Ok(ExchangeConfig {
        private_key,
        api_key,
        network,
    })
}
