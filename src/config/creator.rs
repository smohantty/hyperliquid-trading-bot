use crate::config::strategy::{GridBias, GridType, PerpGridConfig, SpotGridConfig, StrategyConfig};
use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::fs;

pub fn create_config() -> Result<()> {
    let theme = ColorfulTheme::default();

    let strategy_types = vec!["Spot Grid", "Perp Grid"];
    let selection = Select::with_theme(&theme)
        .with_prompt("Select Strategy Type")
        .default(0)
        .items(&strategy_types)
        .interact()?;

    let config = if selection == 0 {
        create_spot_grid(&theme)?
    } else {
        create_perp_grid(&theme)?
    };

    let default_filename = generate_default_filename(&config);

    let filename: String = Input::with_theme(&theme)
        .with_prompt("Configuration filename")
        .default(default_filename)
        .interact_text()?;

    let toml_string = toml::to_string_pretty(&config)?;

    let path = if filename.ends_with(".toml") {
        filename
    } else {
        format!("{}.toml", filename)
    };

    // If directory configs/ exists, maybe suggest putting it there?
    // For now, let's just save relative to CWD, or check if user wants to save in configs/
    // Let's keep it simple: just save where the user said, or prepend configs/ if they want.
    // The requirement didn't specify, so I'll just save to the path they gave.
    // But helpful to save to configs/ by default if they just give a name?
    // Let's check if configs/ exists and prepend it if the path doesn't have a slash.

    let final_path = if !path.contains('/') && fs::metadata("configs").is_ok() {
        format!("configs/{}", path)
    } else {
        path
    };

    fs::write(&final_path, toml_string)?;
    println!("Configuration saved to {}", final_path);

    Ok(())
}

fn create_spot_grid(theme: &ColorfulTheme) -> Result<StrategyConfig> {
    let symbol: String = Input::with_theme(theme)
        .with_prompt("Symbol (e.g., ETH/USDC)")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.ends_with("/USDC") && input != "/USDC" {
                Ok(())
            } else {
                Err("Symbol must be in Asset/USDC format")
            }
        })
        .interact_text()?;

    let grid_range_low: f64 = Input::with_theme(theme)
        .with_prompt("Lower Price")
        .interact_text()?;

    let grid_range_high: f64 = Input::with_theme(theme)
        .with_prompt("Upper Price")
        .validate_with(|input: &f64| -> Result<(), &str> {
            if *input > grid_range_low {
                Ok(())
            } else {
                Err("Upper price must be greater than lower price")
            }
        })
        .interact_text()?;

    let grid_types = vec!["Arithmetic", "Geometric"];
    let grid_type_sel = Select::with_theme(theme)
        .with_prompt("Grid Type")
        .default(0)
        .items(&grid_types)
        .interact()?;

    let grid_type = if grid_type_sel == 0 {
        GridType::Arithmetic
    } else {
        GridType::Geometric
    };

    let grid_count: u32 = Input::with_theme(theme)
        .with_prompt("Grid Count")
        .interact_text()?;

    let total_investment: f64 = Input::with_theme(theme)
        .with_prompt("Total Investment (USDC)")
        .interact_text()?;

    let has_trigger = Confirm::with_theme(theme)
        .with_prompt("Set a Trigger Price?")
        .default(false)
        .interact()?;

    let trigger_price = if has_trigger {
        Some(
            Input::with_theme(theme)
                .with_prompt("Trigger Price")
                .interact_text()?,
        )
    } else {
        None
    };

    Ok(StrategyConfig::SpotGrid(SpotGridConfig {
        symbol,
        grid_range_high,
        grid_range_low,
        grid_type,
        grid_count: Some(grid_count),
        spread_bips: None,
        total_investment,
        trigger_price,
    }))
}

fn create_perp_grid(theme: &ColorfulTheme) -> Result<StrategyConfig> {
    let symbol: String = Input::with_theme(theme)
        .with_prompt("Symbol (e.g., BTC)")
        .interact_text()?;

    let leverage: u32 = Input::with_theme(theme)
        .with_prompt("Leverage (1-50)")
        .validate_with(|input: &u32| -> Result<(), &str> {
            if *input > 0 && *input <= 50 {
                Ok(())
            } else {
                Err("Leverage must be between 1 and 50")
            }
        })
        .interact_text()?;

    let is_isolated = Confirm::with_theme(theme)
        .with_prompt("Isolated Margin?")
        .default(true)
        .interact()?;

    let grid_range_low: f64 = Input::with_theme(theme)
        .with_prompt("Lower Price")
        .interact_text()?;

    let grid_range_high: f64 = Input::with_theme(theme)
        .with_prompt("Upper Price")
        .validate_with(|input: &f64| -> Result<(), &str> {
            if *input > grid_range_low {
                Ok(())
            } else {
                Err("Upper price must be greater than lower price")
            }
        })
        .interact_text()?;

    let grid_types = vec!["Arithmetic", "Geometric"];
    let grid_type_sel = Select::with_theme(theme)
        .with_prompt("Grid Type")
        .default(0)
        .items(&grid_types)
        .interact()?;

    let grid_type = if grid_type_sel == 0 {
        GridType::Arithmetic
    } else {
        GridType::Geometric
    };

    let grid_count: u32 = Input::with_theme(theme)
        .with_prompt("Grid Count")
        .interact_text()?;

    let total_investment: f64 = Input::with_theme(theme)
        .with_prompt("Total Investment (USDC)")
        .interact_text()?;

    let bias_types = vec!["Long", "Short"];
    let bias_sel = Select::with_theme(theme)
        .with_prompt("Grid Bias")
        .default(0)
        .items(&bias_types)
        .interact()?;

    let grid_bias = match bias_sel {
        0 => GridBias::Long,
        1 => GridBias::Short,
        _ => GridBias::Long,
    };

    let has_trigger = Confirm::with_theme(theme)
        .with_prompt("Set a Trigger Price?")
        .default(false)
        .interact()?;

    let trigger_price = if has_trigger {
        Some(
            Input::with_theme(theme)
                .with_prompt("Trigger Price")
                .interact_text()?,
        )
    } else {
        None
    };

    Ok(StrategyConfig::PerpGrid(PerpGridConfig {
        symbol,
        leverage,
        is_isolated,
        grid_range_low,
        grid_range_high,
        grid_type,
        grid_count,
        spread_bips: None,
        total_investment,
        grid_bias,
        trigger_price,
    }))
}

fn generate_default_filename(config: &StrategyConfig) -> String {
    match config {
        StrategyConfig::SpotGrid(SpotGridConfig {
            symbol,
            grid_range_high,
            grid_range_low,
            grid_type,
            ..
        }) => {
            // Extract asset name (e.g., "ETH" from "ETH/USDC")
            let asset = symbol.split('/').next().unwrap_or(symbol);
            format!(
                "{}_Spot_{:?}_{}_{}.toml",
                asset, grid_type, grid_range_low, grid_range_high
            )
        }
        StrategyConfig::PerpGrid(PerpGridConfig {
            symbol,
            leverage,
            grid_bias,
            grid_range_low,
            grid_range_high,
            ..
        }) => {
            format!(
                "{}_Perp_{:?}_{}x_{}_{}.toml",
                symbol, grid_bias, leverage, grid_range_low, grid_range_high
            )
        }
    }
}
