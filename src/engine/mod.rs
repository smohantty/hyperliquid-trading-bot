pub mod context;

use crate::config::exchange::ExchangeConfig;
use crate::config::strategy::StrategyConfig;
use crate::engine::context::{MarketInfo, StrategyContext};
use crate::strategy::Strategy;
use anyhow::Result;
use ethers::signers::LocalWallet;
use hyperliquid_rust_sdk::{BaseUrl, ExchangeClient, InfoClient};
use std::collections::HashMap;

pub struct Engine {
    config: StrategyConfig,
    exchange_config: ExchangeConfig,
}

impl Engine {
    pub fn new(config: StrategyConfig, exchange_config: ExchangeConfig) -> Self {
        Self {
            config,
            exchange_config,
        }
    }

    pub async fn run(self, mut strategy: Box<dyn Strategy>) -> Result<()> {
        println!("Engine started for {}.", self.config.symbol());

        // 1. Configure Base URL
        let base_url = if self.exchange_config.network == "mainnet" {
            BaseUrl::Mainnet
        } else {
            BaseUrl::Testnet
        };

        // 2. Initialize InfoClient
        println!("Connecting to InfoClient...");
        let info_client = InfoClient::new(None, Some(base_url.clone())).await?;

        // 3. Initialize ExchangeClient
        println!("Connecting to ExchangeClient...");
        let wallet: LocalWallet = self.exchange_config.private_key.parse()?;
        let _exchange_client =
            ExchangeClient::new(None, wallet, Some(base_url), None, None).await?;

        // 4. Fetch Metadata (Asset Contexts)
        println!("Fetching market metadata...");
        let mut markets = HashMap::new();

        // --- Fetch and cache Spot Metadata ---
        // Note: We use a block to scope the fetching
        {
            println!("Loading Spot metadata...");
            match info_client.spot_meta().await {
                Ok(spot_meta) => {
                    let index_to_token: HashMap<_, _> =
                        spot_meta.tokens.iter().map(|t| (t.index, t)).collect();
                    for asset in spot_meta.universe {
                        // Construct Pair Name: Base/Quote
                        // Note: Hyperliquid Spot usually pairs with USDC (token 1? or token 0?)
                        // We will assume standard formatting or fallback to asset.name
                        if asset.tokens.len() >= 2 {
                            if let (Some(base), Some(quote)) = (
                                index_to_token.get(&asset.tokens[0]),
                                index_to_token.get(&asset.tokens[1]),
                            ) {
                                let symbol = format!("{}/{}", base.name, quote.name);
                                let sz_decimals = base.sz_decimals as u32;
                                // Spot max decimals = 8
                                let price_decimals = 8u32.saturating_sub(sz_decimals);

                                let info =
                                    MarketInfo::new(symbol.clone(), sz_decimals, price_decimals);
                                markets.insert(symbol, info);

                                // Also insert under internal name if needed?
                                // Or just base name if Quote is USDC?
                                // User code split by '/' to find base.
                            }
                        }
                    }
                }
                Err(e) => println!("Failed to fetch spot metadata: {}", e),
            }
        }

        // --- Fetch and cache Perp Metadata ---
        {
            println!("Loading Perp metadata...");
            match info_client.meta().await {
                Ok(meta) => {
                    for asset in meta.universe {
                        let symbol = asset.name;
                        let sz_decimals = asset.sz_decimals;
                        // Perp max decimals = 6
                        let price_decimals = 6u32.saturating_sub(sz_decimals);

                        let info = MarketInfo::new(symbol.clone(), sz_decimals, price_decimals);
                        markets.insert(symbol, info);
                    }
                }
                Err(e) => println!("Failed to fetch perp metadata: {}", e),
            }
        }

        let target_symbol = self.config.symbol();
        if !markets.contains_key(target_symbol) {
            panic!("Critical Error: Metadata for symbol '{}' not found. Cannot start bot without correct precision/market data.", target_symbol);
        } else {
            println!("Metadata loaded for {}.", target_symbol);
        }

        let mut ctx = StrategyContext::new(markets);

        // 6. Start Event Loop
        println!("Starting Event Loop...");
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("Shutdown signal received. Stopping Engine...");
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                    let mock_price = 2500.0;
                     // In real engine, we update market info in ctx before calling on_tick
                     if let Some(info) = ctx.market_info_mut(target_symbol) {
                         info.last_price = mock_price;
                     }
                    if let Err(e) = strategy.on_tick(mock_price, &mut ctx) {
                        eprintln!("Strategy error: {}", e);
                    }
                }
            }
        }
        println!("Engine stopped gracefully.");
        Ok(())
    }
}
