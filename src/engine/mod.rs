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

struct PendingOrder {
    target_size: f64,
    filled_size: f64,
    weighted_avg_px: f64,
    accumulated_fees: f64,
}

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

        // 1. Initialize InfoClient with automatic reconnection for 24/7 operation
        info!("Connecting to InfoClient...");
        let mut info_client = InfoClient::with_reconnect(None, Some(base_url.clone())).await?;

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

        // --- Fetch Initial Balances ---
        info!("Fetching initial balances...");
        // 1. Fetch Spot Balances
        match info_client.user_token_balances(user_address).await {
            Ok(balances) => {
                for balance in balances.balances {
                    ctx.set_balance(balance.coin, balance.total.parse().unwrap_or(0.0));
                }
            }
            Err(e) => error!("Failed to fetch spot balances: {}", e),
        }
        // 2. Fetch Perp Balances (for USDC margin)
        match info_client.user_state(user_address).await {
            Ok(user_state) => {
                ctx.set_balance(
                    "USDC".to_string(),
                    user_state.withdrawable.parse().unwrap_or(0.0),
                );
            }
            Err(e) => error!("Failed to fetch perp balances (USDC): {}", e),
        }

        for (asset, amount) in &ctx.balances {
            if *amount > 0.0 {
                info!("Balance: {} = {}", asset, amount);
            }
        }

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

        let mut pending_orders: HashMap<u128, PendingOrder> = HashMap::new();
        let mut completed_cloids: std::collections::HashSet<u128> =
            std::collections::HashSet::new(); // Track immediate fills to avoid duplicates

        let mut balance_refresh_timer = tokio::time::interval(std::time::Duration::from_secs(30));

        // 6. Start Event Loop
        info!("Starting Event Loop...");
        loop {
            tokio::select! {
                _ = balance_refresh_timer.tick() => {
                    debug!("Refreshing balances...");
                    let user_addr = user_address;
                    // 1. Fetch Spot Balances
                    match info_client.user_token_balances(user_addr).await {
                        Ok(balances) => {
                            for balance in balances.balances {
                                ctx.set_balance(balance.coin, balance.total.parse().unwrap_or(0.0));
                            }
                        }
                        Err(e) => error!("Periodic: Failed to fetch spot balances: {}", e),
                    }
                    // 2. Fetch Perp Balances (for USDC margin)
                    match info_client.user_state(user_addr).await {
                        Ok(user_state) => {
                            ctx.set_balance(
                                "USDC".to_string(),
                                user_state.withdrawable.parse().unwrap_or(0.0),
                            );
                        }
                        Err(e) => error!("Periodic: Failed to fetch perp balances (USDC): {}", e),
                    }
                }
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
                                        let req_summary = match &order_req {
                                            crate::model::OrderRequest::Limit { is_buy, price, sz, .. } => format!("{} {} @ {}", if *is_buy { "BUY" } else { "SELL" }, sz, price),
                                            crate::model::OrderRequest::Market { is_buy, sz, .. } => format!("MARKET {} {}", if *is_buy { "BUY" } else { "SELL" }, sz),
                                            crate::model::OrderRequest::Cancel { cloid } => format!("CANCEL {}", cloid),
                                        };
                                        info!("Processing Order: {}", req_summary);

                                        match order_req {
                                            crate::model::OrderRequest::Cancel { cloid } => {
                                                info!("Processing Cancel variant in Order Queue for cloid: {}", cloid);
                                                let cancel_req = hyperliquid_rust_sdk::ClientCancelRequestCloid {
                                                    asset: coin.clone(),
                                                    cloid: uuid::Uuid::from_u128(cloid),
                                                };
                                                match _exchange_client.cancel_by_cloid(cancel_req, None).await {
                                                    Ok(res) => info!("Cancel successful: {:?}", res),
                                                    Err(e) => error!("Failed to cancel order {}: {:?}", cloid, e),
                                                }
                                                continue;
                                            }
                                            _ => {}
                                        }

                                        let (is_buy, limit_px, sz, reduce_only, order_type, cloid, target_sz) = match order_req {
                                            crate::model::OrderRequest::Limit { symbol: _, is_buy, price, sz, reduce_only, cloid } => {
                                                (is_buy, price, sz, reduce_only, ClientOrder::Limit(
                                                    ClientLimit {
                                                        tif: "Gtc".to_string(), // Good Till Cancelled
                                                    }
                                                ), cloid, sz)
                                            },
                                            crate::model::OrderRequest::Market { symbol: _, is_buy, sz, cloid } => {
                                                    let aggressive_price = if is_buy {
                                                        mid_price * 1.1
                                                    } else {
                                                        mid_price * 0.9
                                                    };

                                                    // Ensure the aggressive price is rounded to tick size
                                                    let market_info = ctx.market_info(&target_symbol).unwrap();
                                                    let rounded_aggressive_price = market_info.round_price(aggressive_price);

                                                    // Market is often Limit with aggressive price
                                                    (is_buy, rounded_aggressive_price, sz, false, ClientOrder::Limit(
                                                        ClientLimit {
                                                            tif: "Ioc".to_string(), // Immediate or Cancel for Pseudo-Market
                                                        }
                                                    ), cloid, sz)
                                            },
                                            _ => unreachable!("Cancel already handled"),
                                        };

                                        let sdk_req = ClientOrderRequest {
                                            asset: coin.clone(),
                                            is_buy,
                                            limit_px,
                                            sz,
                                            reduce_only,
                                            order_type,
                                            cloid: cloid.map(uuid::Uuid::from_u128),
                                        };

                                         info!("(Live) Sending: {} {} {} @ {}", if sdk_req.is_buy { "BUY" } else { "SELL" }, sdk_req.sz, coin, sdk_req.limit_px);
                                             match _exchange_client.order(sdk_req, None).await {
                                             Ok(res) => {
                                                 // Extract Status Summary
                                                 let mut immediate_fill = None;
                                                 let status_msg = match res {
                                                     hyperliquid_rust_sdk::ExchangeResponseStatus::Ok(exchange_res) => {
                                                         if let Some(data) = &exchange_res.data {
                                                             data.statuses.iter().map(|s| match s {
                                                                 hyperliquid_rust_sdk::ExchangeDataStatus::Resting(r) => format!("Resting (oid: {})", r.oid),
                                                                 hyperliquid_rust_sdk::ExchangeDataStatus::Filled(f) => {
                                                                     // Capture fill details for immediate notification
                                                                     immediate_fill = Some((f.total_sz.clone(), f.avg_px.clone()));
                                                                     format!("Filled (oid: {})", f.oid)
                                                                 },
                                                                 hyperliquid_rust_sdk::ExchangeDataStatus::Error(e) => format!("Error: {}", e),
                                                                 _ => format!("{:?}", s),
                                                             }).collect::<Vec<_>>().join(", ")
                                                         } else {
                                                             format!("{:?}", exchange_res)
                                                         }
                                                     }
                                                     hyperliquid_rust_sdk::ExchangeResponseStatus::Err(e) => format!("Error: {}", e),
                                                 };

                                                 info!("Response: {}", status_msg);

                                                if let Some((total_sz_str, avg_px_str)) = immediate_fill {
                                                    if let Some(c) = cloid {
                                                        let amount: f64 = total_sz_str.parse().unwrap_or(0.0);
                                                        let px: f64 = avg_px_str.parse().unwrap_or(0.0);
                                                        // Note: Fee is unknown in immediate response, assumed 0.0 or handled by reconciliation
                                                        info!("Immediate Fill detected for {}. Notifying strategy.", c);

                                                        let side_str = if is_buy { "B" } else { "S" };
                                                        if let Err(e) = strategy.on_order_filled(side_str, amount, px, 0.0, Some(c), &mut ctx) {
                                                            error!("Strategy on_order_filled error: {}", e);
                                                        }
                                                        completed_cloids.insert(c);
                                                    }
                                                } else {
                                                    // Record in pending_orders if cloid exists AND not immediately filled
                                                    if let Some(c) = cloid {
                                                        pending_orders.insert(c, PendingOrder {
                                                            target_size: target_sz,
                                                            filled_size: 0.0,
                                                            weighted_avg_px: 0.0,
                                                            accumulated_fees: 0.0,
                                                        });
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                error!("Failed to place order: {:?}", e);
                                                if let Some(c) = cloid {
                                                   if let Err(strategy_err) = strategy.on_order_failed(c, &mut ctx) {
                                                       error!("Strategy on_order_failed error: {}", strategy_err);
                                                   }
                                                }
                                            }
                                        }
                                     }

                                     // Execute Cancellation Queue
                                     while let Some(cloid_to_cancel) = ctx.cancellation_queue.pop() {
                                         info!("Processing Cancellation Request for cloid: {}", cloid_to_cancel);
                                         let cancel_req = hyperliquid_rust_sdk::ClientCancelRequestCloid {
                                             asset: coin.clone(),
                                             cloid: uuid::Uuid::from_u128(cloid_to_cancel),
                                         };
                                         match _exchange_client.cancel_by_cloid(cancel_req, None).await {
                                             Ok(res) => info!("Cancel successful: {:?}", res),
                                             Err(e) => error!("Failed to cancel order {}: {:?}", cloid_to_cancel, e),
                                         }
                                     }

                                    }
                                 }
                             },
                         hyperliquid_rust_sdk::Message::User(user_events) => {
                             // Only log if it's not empty and maybe specialize
                             let event_type = match &user_events.data {
                                 UserData::Fills(f) => format!("Fills ({})", f.len()),
                                 UserData::Funding(_) => "Funding".to_string(),
                                 UserData::Liquidation(_) => "Liquidation".to_string(),
                                 UserData::NonUserCancel(c) => format!("NonUserCancel ({})", c.len()),
                             };
                             debug!("User Event: {}", event_type);
                            let user_events_data = user_events.data;
                            if let UserData::Fills(fills) = user_events_data {
                                for fill in fills {
                                    // 1. Filter fills for the current market asset
                                    // Hyperliquid uses "@index" for some assets, let's match the coin string
                                    if fill.coin != coin {
                                        debug!("Ignoring fill for different coin: {} (expected: {})", fill.coin, coin);
                                        continue;
                                    }

                                    let amount: f64 = fill.sz.parse().unwrap_or(0.0);
                                    let px: f64 = fill.px.parse().unwrap_or(0.0);

                                    // 2. Parse cloid with simple hex conversion (satisfies exchange 0x hex)
                                    let cloid = if let Some(cloid_str) = fill.cloid {
                                        let normalized = if cloid_str.starts_with("0x") {
                                            &cloid_str[2..]
                                        } else {
                                            &cloid_str
                                        };
                                        u128::from_str_radix(normalized, 16).ok()
                                    } else {
                                        None
                                    };

                                    // 3. Map fill.dir ("Buy"/"Sell") to "B"/"S"
                                    let side = if fill.dir.to_lowercase().starts_with('b') { "B" } else { "S" };

                                    // Extract fee (if available) - Hyperliquid SDK Fill struct verification needed
                                    // Assuming fill.fee or similar exists. Inspecting based on previous error context or assumption.
                                    // If strict SDK defined, need to check fields. For now, defaulting to 0.0 if field unknown
                                    // But requirement is to parse it. Checking typical structure.
                                    // Since I cannot see SDK source, I will assume `fee` field exists as string or f64.
                                    // If fails compilation, I will inspect SDK.
                                    // "fill" variable is of type Fill.
                                    let fee: f64 = fill.fee.parse().unwrap_or(0.0);

                                    // Pass to strategy or aggregate
                                    if let Some(c) = cloid {
                                        if completed_cloids.contains(&c) {
                                            debug!("Ignored duplicate fill for completed cloid: {}", c);
                                            continue;
                                        }

                                        if let Some(pending) = pending_orders.get_mut(&c) {
                                            let new_total_size = pending.filled_size + amount;
                                            pending.weighted_avg_px = (pending.weighted_avg_px * pending.filled_size + px * amount) / new_total_size;
                                            pending.filled_size = new_total_size;
                                            pending.accumulated_fees += fee;

                                            info!("Order progress for {}: {}/{} filled at avg px {}", c, pending.filled_size, pending.target_size, pending.weighted_avg_px);

                                            if pending.filled_size >= pending.target_size * 0.9999 { // Handle floating point math
                                                info!("Order {} fully filled. Notifying strategy.", c);
                                                let final_px = pending.weighted_avg_px;
                                                let final_sz = pending.filled_size;
                                                let final_fee = pending.accumulated_fees;
                                                pending_orders.remove(&c);

                                                if let Err(e) = strategy.on_order_filled(side, final_sz, final_px, final_fee, Some(c), &mut ctx) {
                                                    error!("Strategy on_order_filled error: {}", e);
                                                }
                                            }
                                        } else {
                                            // Fallback for orders not tracked (e.g. from previous bot run)
                                            info!("Fill for untracked cloid {}. Forwarding immediately.", c);
                                            // For immediate forwarding, pass the fee directly
                                            if let Err(e) = strategy.on_order_filled(side, amount, px, fee, Some(c), &mut ctx) {
                                                error!("Strategy on_order_filled error: {}", e);
                                            }
                                        }
                                    } else {
                                        // No cloid - forward immediately
                                        if let Err(e) = strategy.on_order_filled(side, amount, px, fee, None, &mut ctx) {
                                            error!("Strategy on_order_filled error: {}", e);
                                        }
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
