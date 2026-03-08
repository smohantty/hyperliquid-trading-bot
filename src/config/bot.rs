use crate::config::simulation::SimulationConfig;
use crate::config::strategy::StrategyConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BotConfig {
    pub name: String,
    pub account: String,
    #[serde(default)]
    pub websocket_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation: Option<SimulationConfig>,
    pub strategy: StrategyConfig,
}

impl BotConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.name.trim().is_empty() {
            return Err(anyhow::anyhow!("Bot name must not be empty."));
        }
        if self.account.trim().is_empty() {
            return Err(anyhow::anyhow!("Account profile must not be empty."));
        }
        if let Some(simulation) = &self.simulation {
            simulation.validate()?;
        }
        self.strategy.validate()
    }

    pub fn websocket_port(&self) -> u16 {
        self.websocket_port
            .unwrap_or_else(|| self.strategy.default_websocket_port())
    }

    pub fn simulation_config(&self) -> SimulationConfig {
        self.simulation.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_requires_name_and_account() {
        let config = BotConfig {
            name: "".to_string(),
            account: "account1".to_string(),
            websocket_port: None,
            simulation: None,
            strategy: StrategyConfig::SpotGrid(crate::config::strategy::SpotGridConfig {
                symbol: "BTC/USDC".to_string(),
                grid_range_high: 2000.0,
                grid_range_low: 1000.0,
                grid_type: crate::config::strategy::GridType::Arithmetic,
                grid_count: Some(10),
                spread_bips: None,
                total_investment: 1000.0,
                trigger_price: None,
            }),
        };

        let err = config.validate().unwrap_err().to_string();
        assert_eq!(err, "Bot name must not be empty.");
    }

    #[test]
    fn test_toml_serialization_uses_strategy_table() {
        let config = BotConfig {
            name: "btc-perp-grid".to_string(),
            account: "account1".to_string(),
            websocket_port: Some(9001),
            simulation: None,
            strategy: StrategyConfig::PerpGrid(crate::config::strategy::PerpGridConfig {
                symbol: "BTC".to_string(),
                leverage: 10,
                is_isolated: false,
                grid_range_high: 89500.0,
                grid_range_low: 87000.0,
                grid_type: crate::config::strategy::GridType::Geometric,
                grid_count: Some(20),
                spread_bips: None,
                total_investment: 8000.0,
                grid_bias: crate::config::strategy::GridBias::Short,
                trigger_price: None,
            }),
        };

        let toml = toml::to_string_pretty(&config).unwrap();
        assert!(toml.contains("name = \"btc-perp-grid\""));
        assert!(toml.contains("account = \"account1\""));
        assert!(toml.contains("websocket_port = 9001"));
        assert!(toml.contains("[strategy]"));
    }

    #[test]
    fn test_toml_deserialization_supports_simulation_block() {
        let toml = r#"
name = "hype-spot-grid"
account = "spot_account"

[simulation]
USDC = 5000.0
HYPE = 100.0

[strategy]
type = "spot_grid"
symbol = "HYPE/USDC"
grid_range_high = 24.0
grid_range_low = 20.0
grid_type = "geometric"
grid_count = 40
total_investment = 985.0
"#;

        let config: BotConfig = toml::from_str(toml).unwrap();
        let sim = config.simulation_config();
        assert_eq!(sim.balances.get("USDC"), Some(&5000.0));
        assert_eq!(sim.balances.get("HYPE"), Some(&100.0));
    }

    #[test]
    fn test_spot_strategy_defaults_to_port_8000() {
        let config = BotConfig {
            name: "spot-bot".to_string(),
            account: "account1".to_string(),
            websocket_port: None,
            simulation: None,
            strategy: StrategyConfig::SpotGrid(crate::config::strategy::SpotGridConfig {
                symbol: "BTC/USDC".to_string(),
                grid_range_high: 2000.0,
                grid_range_low: 1000.0,
                grid_type: crate::config::strategy::GridType::Arithmetic,
                grid_count: Some(10),
                spread_bips: None,
                total_investment: 1000.0,
                trigger_price: None,
            }),
        };

        assert_eq!(config.websocket_port(), 8000);
    }

    #[test]
    fn test_perp_strategy_defaults_to_port_8001() {
        let config = BotConfig {
            name: "perp-bot".to_string(),
            account: "account1".to_string(),
            websocket_port: None,
            simulation: None,
            strategy: StrategyConfig::PerpGrid(crate::config::strategy::PerpGridConfig {
                symbol: "BTC".to_string(),
                leverage: 10,
                is_isolated: false,
                grid_range_high: 89500.0,
                grid_range_low: 87000.0,
                grid_type: crate::config::strategy::GridType::Geometric,
                grid_count: Some(20),
                spread_bips: None,
                total_investment: 8000.0,
                grid_bias: crate::config::strategy::GridBias::Short,
                trigger_price: None,
            }),
        };

        assert_eq!(config.websocket_port(), 8001);
    }
}
