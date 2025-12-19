pub mod context;

use crate::config::strategy::StrategyConfig;
use crate::engine::context::{MarketInfo, StrategyContext};
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use ethers::signers::{LocalWallet, Signer};
use hyperliquid_rust_sdk::{BaseUrl, ExchangeClient, InfoClient};
use log::{debug, error, info};
use std::collections::HashMap;

// Updated imports based on documentation discovery
use hyperliquid_rust_sdk::{ClientLimit, ClientOrder, ClientOrderRequest, UserData};

pub struct Engine {
    config: StrategyConfig,
    exchange_config: crate::config::exchange::ExchangeConfig,
}

impl Engine {
    pub fn new(
        config: StrategyConfig,
        exchange_config: crate::config::exchange::ExchangeConfig,
    ) -> Self {
        Self {
            config,
            exchange_config,
        }
    }

    pub async fn run(&self, mut strategy: Box<dyn Strategy>) -> Result<()> {
        info!("Engine started for {}.", self.config.symbol());

        let private_key = &self.exchange_config.private_key;
        let wallet: LocalWallet = private_key
            .parse()
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;

        let base_url = if self.exchange_config.network == "mainnet" {
            BaseUrl::Mainnet
        } else {
            BaseUrl::Testnet
        };

        // 1. Initialize InfoClient
        info!("Connecting to InfoClient...");
        let mut info_client = InfoClient::new(None, Some(base_url.clone())).await?;

        // 2. Initialize ExchangeClient
        info!("Connecting to ExchangeClient...");
        let user_address = wallet.address();
        let _exchange_client =
            ExchangeClient::new(None, wallet, Some(base_url.clone()), None, None).await?;

        // 4. Fetch Metadata (Asset Contexts)
        info!("Fetching market metadata...");
        let mut markets = HashMap::new();

        // --- Fetch and cache Spot Metadata ---
        {
            debug!("Loading Spot metadata...");
            match info_client.spot_meta().await {
                Ok(spot_meta) => {
                    let index_to_token: HashMap<_, _> =
                        spot_meta.tokens.iter().map(|t| (t.index, t)).collect();
                    for asset in spot_meta.universe {
                        if asset.tokens.len() >= 2 {
                            if let (Some(base), Some(quote)) = (
                                index_to_token.get(&asset.tokens[0]),
                                index_to_token.get(&asset.tokens[1]),
                            ) {
                                let symbol = format!("{}/{}", base.name, quote.name);
                                let coin = asset.name.clone();
                                let asset_index = asset.index as u32;

                                if symbol == self.config.symbol() {
                                    debug!(
                                        "Found Spot Market: {} (Coin: {}, Index: {})",
                                        symbol, coin, asset_index
                                    );
                                }
                                let sz_decimals = base.sz_decimals as u32;
                                let price_decimals = 8u32.saturating_sub(sz_decimals);

                                let info = MarketInfo::new(
                                    symbol.clone(),
                                    coin,
                                    asset_index,
                                    sz_decimals,
                                    price_decimals,
                                );
                                markets.insert(symbol, info);
                            }
                        }
                    }
                }
                Err(e) => error!("Failed to fetch spot metadata: {}", e),
            }
        }

        // --- Fetch and cache Perp Metadata ---
        {
            debug!("Loading Perp metadata...");
            match info_client.meta().await {
                Ok(meta) => {
                    for (i, asset) in meta.universe.iter().enumerate() {
                        let symbol = asset.name.clone();
                        let coin = symbol.clone();
                        let asset_index = i as u32;

                        let sz_decimals = asset.sz_decimals;
                        let price_decimals = 6u32.saturating_sub(sz_decimals);

                        let info = MarketInfo::new(
                            symbol.clone(),
                            coin,
                            asset_index,
                            sz_decimals,
                            price_decimals,
                        );
                        markets.insert(symbol, info);
                    }
                }
                Err(e) => error!("Failed to fetch perp metadata: {}", e),
            }
        }

        let target_symbol = self.config.symbol();
        if !markets.contains_key(target_symbol) {
            panic!(
                "Critical Error: Metadata for symbol '{}' not found.",
                target_symbol
            );
        } else {
            info!("Metadata loaded for {}.", target_symbol);
        }

        let mut ctx = StrategyContext::new(markets);

        // Retrieve context for subscription
        let market_info = ctx.market_info(target_symbol).unwrap();
        info!(
            "Asset Precision for {}: Size Decimals = {}, Price Decimals = {}",
            target_symbol, market_info.sz_decimals, market_info.price_decimals
        );
        let coin = market_info.coin.clone();
        let _asset_index = market_info.asset_index;

        // 5. Subscribe to WebSockets
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        // Subscribe to AllMids instead of L2Book as requested
        info_client
            .subscribe(hyperliquid_rust_sdk::Subscription::AllMids, sender.clone())
            .await
            .map_err(|e| anyhow!("Failed to subscribe to AllMids: {}", e))?;
        info!("Subscribed to AllMids.");

        info_client
            .subscribe(
                hyperliquid_rust_sdk::Subscription::UserEvents { user: user_address },
                sender,
            )
            .await
            .map_err(|e| anyhow!("Failed to subscribe to UserEvents: {}", e))?;
        info!("Subscribed to UserEvents for {:?}.", user_address);

        // 6. Start Event Loop
        info!("Starting Event Loop...");
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutdown signal received. Stopping Engine...");
                    break;
                }
                Some(message) = receiver.recv() => {
                    match message {
                        hyperliquid_rust_sdk::Message::AllMids(all_mids) => {
                             if let Some(price_str) = all_mids.data.mids.get(&coin) {
                                 let mid_price = price_str.parse::<f64>().unwrap_or(0.0);
                                 if mid_price > 0.0 {
                                     // Update Market Info
                                     if let Some(info) = ctx.market_info_mut(&target_symbol) {
                                         info.last_price = mid_price;
                                     }

                                     // Call Strategy
                                     if let Err(e) = strategy.on_tick(mid_price, &mut ctx) {
                                         error!("Strategy error: {}", e);
                                     }

                                     // Execute Order Queue
                                     while let Some(order_req) = ctx.order_queue.pop() {
                                        info!("Processing Order Request: {:?}", order_req);

                                        // Mapping to SDK types
                                        let (is_buy, limit_px, sz, reduce_only, order_type) = match order_req {
                                            crate::model::OrderRequest::Limit { symbol: _, is_buy, price, sz, reduce_only } => {
                                                (is_buy, price, sz, reduce_only, ClientOrder::Limit(
                                                    ClientLimit {
                                                        tif: "Gtc".to_string(), // Good Till Cancelled
                                                    }
                                                ))
                                            },
                                            crate::model::OrderRequest::Market { symbol: _, is_buy, sz } => {
                                                    let aggressive_price = if is_buy {
                                                        mid_price * 1.1
                                                    } else {
                                                        mid_price * 0.9
                                                    };
                                                    // Market is often Limit with aggressive price
                                                    (is_buy, aggressive_price, sz, false, ClientOrder::Limit(
                                                        ClientLimit {
                                                            tif: "Ioc".to_string(), // Immediate or Cancel for Pseudo-Market
                                                        }
                                                    ))
                                            }
                                        };

                                        let sdk_req = ClientOrderRequest {
                                            asset: coin.clone(),
                                            is_buy,
                                            limit_px,
                                            sz,
                                            reduce_only,
                                            order_type,
                                            cloid: None,
                                        };

                                        info!("(Safe Mode) Order Simulation: {:?}", sdk_req);
                                        // Note: Actual execution commented out for safety per user request.
                                        // exchange_client.order(sdk_req, None).await...
                                     }
                                 }
                             }
                        },
                        hyperliquid_rust_sdk::Message::User(user_events) => {
                            debug!("User Event: {:?}", user_events);
                            let user_events_data = user_events.data;
                            if let UserData::Fills(fills) = user_events_data {
                                for fill in fills {
                                    let amount: f64 = fill.sz.parse().unwrap_or(0.0);
                                    let px: f64 = fill.px.parse().unwrap_or(0.0);
                                    // SDK's Fill struct has 'side' as String (e.g., "B", "A", or "S"?)
                                    // Logging indicates "Fill: bought..." or "sold..."
                                    // Pass to strategy
                                    if let Err(e) = strategy.on_order_filled(&fill.side, amount, px, &mut ctx) {
                                        error!("Strategy on_order_filled error: {}", e);
                                    }
                                }
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
        info!("Engine stopped gracefully.");
        Ok(())
    }
}
