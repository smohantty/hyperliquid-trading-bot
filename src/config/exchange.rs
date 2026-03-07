use anyhow::{anyhow, Context, Result};
use ethers::types::H160;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

const DEFAULT_ACCOUNTS_DIR: &str = "hyperliquid";
const DEFAULT_ACCOUNTS_FILE: &str = "accounts.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct AccountsConfig {
    pub accounts: HashMap<String, AccountProfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountProfile {
    pub network: String,
    pub master_account_address: String,
    pub sub_account_address: Option<String>,
    pub api_wallet_private_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub account_name: String,
    pub network: String,
    pub master_account_address: String,
    pub sub_account_address: Option<String>,
    pub api_wallet_private_key: String,
}

impl ExchangeConfig {
    pub fn trading_account_address(&self) -> &str {
        self.sub_account_address
            .as_deref()
            .unwrap_or(&self.master_account_address)
    }

    pub fn vault_address(&self) -> Option<&str> {
        self.sub_account_address.as_deref()
    }
}

fn validate_address(label: &str, address: &str) -> Result<()> {
    H160::from_str(address).map_err(|e| anyhow!("Invalid {} '{}': {}", label, address, e))?;
    Ok(())
}

fn validate_network(network: &str) -> Result<String> {
    let normalized = network.trim().to_lowercase();
    match normalized.as_str() {
        "mainnet" | "testnet" => Ok(normalized),
        _ => Err(anyhow!(
            "Invalid network '{}'. Expected 'mainnet' or 'testnet'.",
            network
        )),
    }
}

fn default_accounts_file_path() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME must be set to resolve the default accounts file")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join(DEFAULT_ACCOUNTS_DIR)
        .join(DEFAULT_ACCOUNTS_FILE))
}

pub fn resolve_accounts_file_path(override_path: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(PathBuf::from(path));
    }

    default_accounts_file_path()
}

pub fn load_exchange_config(
    account_name: &str,
    accounts_file_override: Option<&str>,
) -> Result<ExchangeConfig> {
    let accounts_file_path = resolve_accounts_file_path(accounts_file_override)?;

    let content = fs::read_to_string(&accounts_file_path).with_context(|| {
        format!(
            "Failed to read accounts file at {}",
            accounts_file_path.display()
        )
    })?;

    let accounts_config: AccountsConfig =
        toml::from_str(&content).with_context(|| "Failed to parse accounts TOML")?;

    let profile = accounts_config
        .accounts
        .get(account_name)
        .with_context(|| format!("Account profile '{}' not found", account_name))?;

    let network = validate_network(&profile.network)?;
    validate_address("master_account_address", &profile.master_account_address)?;
    if let Some(sub_account) = &profile.sub_account_address {
        validate_address("sub_account_address", sub_account)?;
    }

    let api_wallet_private_key = profile.api_wallet_private_key.trim().to_string();

    if api_wallet_private_key.is_empty() {
        return Err(anyhow!(
            "api_wallet_private_key for account '{}' is empty",
            account_name
        ));
    }

    Ok(ExchangeConfig {
        account_name: account_name.to_string(),
        network,
        master_account_address: profile.master_account_address.clone(),
        sub_account_address: profile.sub_account_address.clone(),
        api_wallet_private_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn load_exchange_config_resolves_named_account() {
        let _guard = ENV_MUTEX.lock().unwrap();

        let dir = tempdir().unwrap();
        let accounts_path = dir.path().join("accounts.toml");

        let accounts_toml = r#"
[accounts.account1]
network = "mainnet"
master_account_address = "0x1111111111111111111111111111111111111111"
sub_account_address = "0x2222222222222222222222222222222222222222"
api_wallet_private_key = "0xmainnet-secret"
"#;
        fs::write(&accounts_path, accounts_toml).unwrap();

        let cfg = load_exchange_config("account1", Some(accounts_path.to_string_lossy().as_ref()))
            .unwrap();
        assert_eq!(cfg.account_name, "account1");
        assert_eq!(cfg.network, "mainnet");
        assert_eq!(
            cfg.trading_account_address(),
            "0x2222222222222222222222222222222222222222"
        );
        assert_eq!(
            cfg.vault_address(),
            Some("0x2222222222222222222222222222222222222222")
        );
        assert_eq!(cfg.api_wallet_private_key, "0xmainnet-secret");
    }
}
