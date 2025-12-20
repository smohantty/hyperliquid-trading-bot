use anyhow::Result;
use clap::Parser;
use hyperliquid_trading_bot::config::{exchange::load_exchange_config, load_config};
use hyperliquid_trading_bot::engine::Engine;
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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,hyperliquid_trading_bot=debug"),
    )
    .format(|buf, record| {
        use std::io::Write;
        writeln!(
            buf,
            "[{} {} {}] {}",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
            record.level(),
            record.target(),
            record.args()
        )
    })
    .init();
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

    info!(
        "Starting {} Strategy for {}",
        config.type_name(),
        config.symbol()
    );

    // Default port 9000 if not specified
    let ws_port = args.ws_port.or(Some(9000));
    if let Some(p) = ws_port {
        info!("WebSocket Status Server enabled on port {}", p);
    }

    // Initialize Strategy
    let strategy = init_strategy(config.clone());

    // Initialize Engine
    let engine = Engine::new(config, exchange_config, ws_port);

    // Run Engine
    if let Err(e) = engine.run(strategy).await {
        error!("Engine error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
