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

    // Initialize strategy
    let strategy = init_strategy(config);

    // Run strategy
    match strategy.run() {
        Ok(_) => println!("Strategy finished successfully."),
        Err(e) => eprintln!("Strategy error: {}", e),
    }

    Ok(())
}
