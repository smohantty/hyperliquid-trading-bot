use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for simulation/dry-run mode.
///
/// This block lives inside the main bot TOML under `[simulation]`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SimulationConfig {
    /// Per-asset balances applied on top of real fetched balances.
    /// Each key is an asset symbol (e.g., "USDC", "HYPE").
    #[serde(default, flatten)]
    pub balances: HashMap<String, f64>,
}

impl SimulationConfig {
    pub fn validate(&self) -> Result<()> {
        for (asset, balance) in &self.balances {
            if balance.is_sign_negative() {
                return Err(anyhow!(
                    "Simulation config contains a negative balance for '{}'.",
                    asset
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SimulationConfig::default();
        assert!(config.balances.is_empty());
    }

    #[test]
    fn test_parse_from_toml() {
        let toml = r#"
USDC = 5000.0
HYPE = 100.0
"#;
        let config: SimulationConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.balances.get("USDC"), Some(&5000.0));
        assert_eq!(config.balances.get("HYPE"), Some(&100.0));
    }

    #[test]
    fn test_validate_rejects_negative_override_balance() {
        let config = SimulationConfig {
            balances: HashMap::from([(String::from("USDC"), -1.0)]),
        };

        let err = config.validate().unwrap_err().to_string();
        assert_eq!(
            err,
            "Simulation config contains a negative balance for 'USDC'."
        );
    }
}
