use clap::Parser;
use hyperliquid_trading_bot::config::load_config;
use hyperliquid_trading_bot::strategy::init_strategy;

#[derive(Parser, Debug)]
#[command(author, version, about = "Hyperliquid Trading Bot", long_about = None)]
struct Args {
    #[arg(short, long)]
    config: Option<String>,

    #[arg(short, long)]
    list_strategies: bool,

    #[arg(long)]
    create: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.create {
        if let Err(e) = hyperliquid_trading_bot::config::creator::create_config() {
            eprintln!("Error creating config: {}", e);
        }
        return Ok(());
    }

    if args.list_strategies {
        hyperliquid_trading_bot::config::strategy::print_strategy_help();
        return Ok(());
    }

    let config_path = args.config.ok_or_else(|| {
        anyhow::anyhow!("Config file is required unless --list-strategies or --create is used")
    })?;

    // Load configuration
    println!("Loading config from: {}", config_path);
    let config = load_config(&config_path)?;

    // Load exchange configuration
    let exchange_config = match hyperliquid_trading_bot::config::exchange::load_exchange_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: Failed to load exchange config: {}", e);
            // We might want to exit here if keys are strictly required, but for now just warn.
            // Actually, for a trading bot, keys ARE likely required.
            // Let's propagate error?
            // User requirement said "can be used by the bot", implying it's essential.
            return Err(anyhow::anyhow!("Failed to load exchange config: {}", e));
        }
    };
    println!(
        "Exchange config loaded for network: {}",
        exchange_config.network
    );

    println!(
        "Starting {} Strategy for {}",
        config.type_name(),
        config.symbol()
    );

    // Initialize strategy
    let strategy = init_strategy(config.clone()); // config is cheap to clone or we reference it, but init consumes it. StrategyConfig is Clone derived.

    // Initialize Engine
    let engine = hyperliquid_trading_bot::engine::Engine::new(config, exchange_config);

    // Run the engine
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { engine.run(strategy).await })?;

    Ok(())
}
