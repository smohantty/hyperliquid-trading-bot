use anyhow::Result;
use clap::Parser;
use hyperliquid_trading_bot::broadcast::StatusBroadcaster;
use hyperliquid_trading_bot::config::broadcast::load_broadcast_config;
use hyperliquid_trading_bot::config::{exchange::load_exchange_config, load_config};
use hyperliquid_trading_bot::engine::Engine;
use hyperliquid_trading_bot::reporter::telegram::TelegramReporter;
use hyperliquid_trading_bot::strategy::init_strategy;
use log::{error, info}; // Keep this import

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
    ws_port: Option<u16>,
}

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

#[tokio::main]
async fn main() -> Result<()> {
    // ---------------------------------------------------------
    // 1. Setup Logging (Tracing)
    // ---------------------------------------------------------
    let file_appender = tracing_appender::rolling::daily("logs", "application.log");
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

    // File Layer (JSON)
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking)
        .json()
        .with_filter(tracing_subscriber::EnvFilter::new(
            "info,hyperliquid_trading_bot=debug",
        ));

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    // ---------------------------------------------------------
    // 2. Setup Audit Logger
    // ---------------------------------------------------------
    let audit_logger =
        match hyperliquid_trading_bot::logging::order_audit::OrderAuditLogger::new("logs") {
            Ok(l) => Some(l),
            Err(e) => {
                error!("Failed to initialize Order Audit Logger: {}", e);
                None
            }
        };

    let args = Args::parse();

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
    let config = load_config(&config_path)?;

    // Load exchange configuration
    let exchange_config = match load_exchange_config() {
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

    // Load broadcast configuration (Telegram & WebSocket)
    let broadcast_config = match load_broadcast_config(args.ws_port) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load broadcast config: {}", e);
            std::process::exit(1);
        }
    };

    info!(
        "Starting {} Strategy for {}",
        config.type_name(),
        config.symbol()
    );

    // Default port 9000 if not specified
    let ws_config = Some(broadcast_config.websocket.clone());
    let broadcaster = StatusBroadcaster::new(ws_config.clone());
    if let Some(conf) = ws_config {
        info!(
            "WebSocket Status Server enabled on {}:{}",
            conf.host, conf.port
        );
    }

    // Initialize Telegram Reporter
    let mut reporter_handle = None;
    if let Some(telegram_config) = broadcast_config.telegram {
        match TelegramReporter::new(broadcaster.subscribe(), telegram_config, config.clone()) {
            Ok(reporter) => {
                info!("Telegram Reporter initialized. Spawning background task...");
                reporter_handle = Some(tokio::spawn(reporter.run()));
            }
            Err(e) => {
                error!("Failed to initialize Telegram Reporter: {}", e);
            }
        }
    }

    // Initialize Strategy
    let strategy = match init_strategy(config.clone()) {
        Ok(s) => s,
        Err(e) => {
            error!("Strategy initialization failed: {}", e);
            // Broadcast Error to Reporters
            broadcaster.send(hyperliquid_trading_bot::broadcast::types::WSEvent::Error(
                e.to_string(),
            ));

            if let Some(handle) = reporter_handle {
                info!("Waiting for Telegram Reporter to shut down...");
                let _ = tokio::time::timeout(std::time::Duration::from_secs(10), handle).await;
            } else {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            std::process::exit(1);
        }
    };

    // Initialize Engine
    let engine = Engine::new(config, exchange_config, broadcaster.clone(), audit_logger);

    // Run Engine
    if let Err(e) = engine.run(strategy).await {
        error!("Engine error: {}", e);
        // Broadcast Error to Reporters
        broadcaster.send(hyperliquid_trading_bot::broadcast::types::WSEvent::Error(
            e.to_string(),
        ));

        // Wait for Telegram Reporter to finish sending the message
        if let Some(handle) = reporter_handle {
            info!("Waiting for Telegram Reporter to shut down...");
            // Allow up to 10 seconds for the message to send
            let _ = tokio::time::timeout(std::time::Duration::from_secs(10), handle).await;
        } else {
            // Fallback if no reporter
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        std::process::exit(1);
    }

    Ok(())
}
