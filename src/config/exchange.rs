use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct AgentPrivateKeys {
    pub mainnet: String,
    pub testnet: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletConfig {
    pub master_account_address: String,
    pub agent_private_key: AgentPrivateKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub private_key: String,
    pub master_account_address: String,
    pub network: String,
}

pub fn load_exchange_config() -> Result<ExchangeConfig> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

    let config_path = env::var("HYPERLIQUID_WALLET_CONFIG_FILE")
        .context("HYPERLIQUID_WALLET_CONFIG_FILE environment variable must be set")?;

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read wallet config file at {}", config_path))?;

    let wallet_config: WalletConfig =
        serde_json::from_str(&content).with_context(|| "Failed to parse wallet config JSON")?;

    let network = env::var("HYPERLIQUID_NETWORK").unwrap_or_else(|_| "mainnet".to_string());

    let private_key = if network == "mainnet" {
        wallet_config.agent_private_key.mainnet
    } else {
        wallet_config.agent_private_key.testnet
    };

    Ok(ExchangeConfig {
        private_key,
        master_account_address: wallet_config.master_account_address,
        network,
    })
}
