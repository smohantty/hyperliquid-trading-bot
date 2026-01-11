use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

/// Balance mode for simulation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BalanceMode {
    /// Fetch actual balances from exchange
    Real,
    /// Use unlimited fake balances (default)
    #[default]
    Unlimited,
    /// Fetch real balances, then override specific assets
    Override,
}

/// Configuration for simulation/dry-run mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    /// Balance mode: "real", "unlimited", or "override"
    #[serde(default)]
    pub balance_mode: BalanceMode,

    /// Per-asset balance overrides (used when balance_mode is "override")
    /// Maps asset symbol (e.g., "USDC", "HYPE") to balance amount
    #[serde(default, alias = "balances")]
    pub balance_overrides: HashMap<String, f64>,

    /// Default balance for unlimited mode
    #[serde(default = "default_unlimited_amount")]
    pub unlimited_amount: f64,
}

fn default_unlimited_amount() -> f64 {
    1_000_000.0
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            balance_mode: BalanceMode::Unlimited,
            balance_overrides: HashMap::new(),
            unlimited_amount: default_unlimited_amount(),
        }
    }
}

/// Load simulation configuration from a JSON file.
///
/// Resolution order:
/// 1. Explicit path argument
/// 2. HYPERLIQUID_SIMULATION_CONFIG_FILE environment variable
/// 3. Default: "simulation_config.json" in current directory
///
/// If the file doesn't exist, returns default configuration.
pub fn load_simulation_config(path: Option<&str>) -> SimulationConfig {
    let config_path = path
        .map(String::from)
        .or_else(|| env::var("HYPERLIQUID_SIMULATION_CONFIG_FILE").ok())
        .unwrap_or_else(|| "simulation_config.json".to_string());

    // If file doesn't exist, return defaults
    if !Path::new(&config_path).exists() {
        log::info!(
            "Simulation config not found at '{}', using defaults (unlimited mode)",
            config_path
        );
        return SimulationConfig::default();
    }

    match fs::read_to_string(&config_path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(mut json) => {
                // Filter out comment keys (those starting with "//")
                if let Some(obj) = json.as_object_mut() {
                    obj.retain(|k, _| !k.starts_with("//"));
                }

                match serde_json::from_value(json) {
                    Ok(config) => {
                        log::info!("Loaded simulation config from '{}'", config_path);
                        config
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to parse simulation config from '{}': {}. Using defaults.",
                            config_path,
                            e
                        );
                        SimulationConfig::default()
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to parse JSON from '{}': {}. Using defaults.",
                    config_path,
                    e
                );
                SimulationConfig::default()
            }
        },
        Err(e) => {
            log::warn!(
                "Failed to read simulation config from '{}': {}. Using defaults.",
                config_path,
                e
            );
            SimulationConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SimulationConfig::default();
        assert_eq!(config.balance_mode, BalanceMode::Unlimited);
        assert_eq!(config.unlimited_amount, 1_000_000.0);
        assert!(config.balance_overrides.is_empty());
    }

    #[test]
    fn test_parse_balance_mode() {
        let json = r#"{"balance_mode": "override", "balances": {"USDC": 5000.0}}"#;
        let config: SimulationConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.balance_mode, BalanceMode::Override);
        assert_eq!(config.balance_overrides.get("USDC"), Some(&5000.0));
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let config = load_simulation_config(Some("/nonexistent/path.json"));
        assert_eq!(config.balance_mode, BalanceMode::Unlimited);
    }
}
