use anyhow::Result;
use clap::Parser;
use hyperliquid_trading_bot::broadcast::StatusBroadcaster;
use hyperliquid_trading_bot::config::bot::BotConfig;
use hyperliquid_trading_bot::config::broadcast::load_broadcast_config;
use hyperliquid_trading_bot::config::exchange::ExchangeConfig;
use hyperliquid_trading_bot::config::{exchange::load_exchange_config, load_bot_config};
use hyperliquid_trading_bot::engine::simulation::SimulationEngine;
use hyperliquid_trading_bot::engine::Engine;
use hyperliquid_trading_bot::strategy::init_strategy;
use hyperliquid_trading_bot::ui::console::ConsoleRenderer;
use log::{error, info}; // Keep this import
use std::backtrace::Backtrace;

#[derive(Parser, Debug)]
#[command(author, version, about = "Hyperliquid Trading Bot", long_about = None)]
struct Args {
    #[arg(short, long)]
    config: Option<String>,

    #[arg(short, long)]
    list_strategies: bool,

    #[arg(long)]
    create: bool,

    #[arg(long)]
    accounts_file: Option<String>,

    /// Run in simulation mode (dry run preview)
    #[arg(long)]
    dry_run: bool,
}

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

fn log_file_name(dry_run: bool) -> &'static str {
    if dry_run {
        "simulation.log"
    } else {
        "application.log"
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // ---------------------------------------------------------
    // 1. Setup Logging (Tracing)
    // ---------------------------------------------------------
    let file_appender = tracing_appender::rolling::daily("logs", log_file_name(args.dry_run));
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Console Layer (Env Filter)
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_level(true)
        .with_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
                .add_directive("hyperliquid_trading_bot=debug".parse().unwrap()),
        );

    // File Layer (Simple Text)
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking)
        .with_target(false)
        .with_filter(tracing_subscriber::EnvFilter::new(
            "info,hyperliquid_trading_bot=debug",
        ));

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| "unknown".to_string());
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "non-string panic payload"
        };
        let backtrace = Backtrace::force_capture();
        error!(
            "Unhandled panic at {}: {}\nbacktrace:\n{}",
            location, payload, backtrace
        );
    }));

    info!(
        "Initialized logging for mode={} file=logs/{}.*",
        if args.dry_run { "simulation" } else { "live" },
        log_file_name(args.dry_run)
    );

    if args.list_strategies {
        hyperliquid_trading_bot::config::strategy::print_strategy_help();
        return Ok(());
    }

    if args.create {
        if let Err(e) = hyperliquid_trading_bot::config::creator::create_config() {
            error!("Error creating config: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    let config_path = args.config.ok_or_else(|| {
        anyhow::anyhow!("Config file is required unless --list-strategies or --create is used")
    })?;

    // Load configuration
    info!("Loading config from: {}", config_path);
    let bot_config = load_bot_config(&config_path)?;

    // Load exchange configuration
    let exchange_config =
        match load_exchange_config(&bot_config.account, args.accounts_file.as_deref()) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to load exchange config: {}", e);
                std::process::exit(1);
            }
        };
    info!(
        "Exchange config loaded for network: {}",
        exchange_config.network
    );

    // --- DRY RUN MODE ---
    if args.dry_run {
        info!("[SIMULATION] Running in dry-run mode...");
        return run_simulation(bot_config, exchange_config).await;
    }

    // --- LIVE TRADING MODE ---
    // Load broadcast configuration (WebSocket)
    let broadcast_config = load_broadcast_config(bot_config.websocket_port());

    info!(
        "Starting bot '{}' with {} Strategy for {} on account '{}' (ws port {})",
        bot_config.name,
        bot_config.strategy.type_name(),
        bot_config.strategy.symbol(),
        bot_config.account,
        bot_config.websocket_port()
    );

    let ws_config = Some(broadcast_config.websocket.clone());
    let broadcaster = StatusBroadcaster::new(ws_config.clone());
    if let Some(conf) = ws_config {
        info!(
            "WebSocket Status Server enabled on {}:{}",
            conf.host, conf.port
        );
    }

    // Initialize Strategy
    let strategy = match init_strategy(bot_config.strategy.clone()) {
        Ok(s) => s,
        Err(e) => {
            error!("Strategy initialization failed: {}", e);
            // Broadcast Error to WebSocket clients
            broadcaster.send(hyperliquid_trading_bot::broadcast::types::WSEvent::Error(
                e.to_string(),
            ));
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            std::process::exit(1);
        }
    };

    // Initialize Engine
    let engine = Engine::new(bot_config.strategy, exchange_config, broadcaster.clone());

    // Run Engine
    if let Err(e) = engine.run(strategy).await {
        error!("Engine error: {}", e);
        // Broadcast Error to WebSocket clients
        broadcaster.send(hyperliquid_trading_bot::broadcast::types::WSEvent::Error(
            e.to_string(),
        ));
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::log_file_name;

    #[test]
    fn test_log_file_name_for_live_mode() {
        assert_eq!(log_file_name(false), "application.log");
    }

    #[test]
    fn test_log_file_name_for_dry_run_mode() {
        assert_eq!(log_file_name(true), "simulation.log");
    }
}

/// Run simulation (dry run) mode.
async fn run_simulation(bot_config: BotConfig, exchange_config: ExchangeConfig) -> Result<()> {
    let sim_config = bot_config.simulation_config();
    info!(
        "[SIMULATION] Starting dry-run with {} simulation balance patch(es)",
        sim_config.balances.len()
    );

    // Initialize strategy
    let mut strategy = match init_strategy(bot_config.strategy.clone()) {
        Ok(s) => s,
        Err(e) => {
            error!("Strategy initialization failed: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize simulation engine
    let mut engine =
        SimulationEngine::new(bot_config.strategy.clone(), exchange_config, sim_config);

    if let Err(e) = engine.initialize().await {
        error!("Simulation engine initialization failed: {}", e);
        std::process::exit(1);
    }

    // Run single step
    let current_price = match engine.run_single_step(&mut strategy).await {
        Ok(price) => price,
        Err(e) => {
            error!("Simulation run failed: {}", e);
            std::process::exit(1);
        }
    };

    // Render results
    ConsoleRenderer::render(
        engine.config(),
        engine.get_summary(strategy.as_ref()).as_ref(),
        engine.get_grid_state(strategy.as_ref()).as_ref(),
        &engine.get_orders(),
        Some(current_price),
    );

    Ok(())
}
