pub mod context;

use crate::config::strategy::StrategyConfig;
use crate::engine::context::{MarketInfo, StrategyContext};
use crate::model::{Cloid, OrderFill, OrderSide};
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use ethers::signers::{LocalWallet, Signer};
use hyperliquid_rust_sdk::{BaseUrl, ExchangeClient, InfoClient};
use std::collections::{HashMap, HashSet};
use tracing::{debug, error, info};

// Updated imports based on documentation discovery
use hyperliquid_rust_sdk::{ClientLimit, ClientOrder, ClientOrderRequest, UserData};

struct PendingOrder {
    target_size: f64,
    filled_size: f64,
    weighted_avg_px: f64,
    accumulated_fees: f64,
    reduce_only: bool,
}

struct EngineRuntime {
    pub ctx: StrategyContext,
    pub pending_orders: HashMap<Cloid, PendingOrder>,
    pub completed_cloids: HashSet<Cloid>,
}

impl EngineRuntime {
    fn new(ctx: StrategyContext) -> Self {
        Self {
            ctx,
            pending_orders: HashMap::new(),
            completed_cloids: HashSet::new(),
        }
    }
}

use crate::broadcast::types::StrategySummary;
use crate::broadcast::{MarketEvent, OrderEvent, StatusBroadcaster, WSEvent};

// ... existing imports ...

use crate::logging::order_audit::OrderAuditLogger;

pub struct Engine {
    config: StrategyConfig,
    exchange_config: crate::config::exchange::ExchangeConfig,
    broadcaster: StatusBroadcaster,
    audit_logger: Option<OrderAuditLogger>, // Optional to allow running without it if needed, but we'll enforce it in main
}

impl Engine {
    pub fn new(
        config: StrategyConfig,
        exchange_config: crate::config::exchange::ExchangeConfig,
        broadcaster: StatusBroadcaster,
        audit_logger: Option<OrderAuditLogger>,
    ) -> Self {
        Self {
            config,
            exchange_config,
            broadcaster,
            audit_logger,
        }
    }

    async fn setup_info_client(&self) -> Result<InfoClient> {
        let base_url = if self.exchange_config.network == "mainnet" {
            BaseUrl::Mainnet
        } else {
            BaseUrl::Testnet
        };
        info!("Connecting to InfoClient...");
        InfoClient::with_reconnect(None, Some(base_url))
            .await
            .map_err(|e| anyhow!("Failed to connect InfoClient: {}", e))
    }

    async fn setup_exchange_client(&self, wallet: LocalWallet) -> Result<ExchangeClient> {
        let base_url = if self.exchange_config.network == "mainnet" {
            BaseUrl::Mainnet
        } else {
            BaseUrl::Testnet
        };
        info!("Connecting to ExchangeClient...");
        ExchangeClient::new(None, wallet, Some(base_url), None, None)
            .await
            .map_err(|e| anyhow!("Failed to connect ExchangeClient: {}", e))
    }

    async fn load_metadata(
        &self,
        info_client: &mut InfoClient,
    ) -> Result<HashMap<String, MarketInfo>> {
        info!("Fetching market metadata...");
        let mut markets = HashMap::new();

        // --- Fetch and cache Spot Metadata ---
        {
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

                                if symbol == self.config.symbol() {}
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

        Ok(markets)
    }

    async fn fetch_balances(
        &self,
        info_client: &mut InfoClient,
        user_address: ethers::types::H160,
        ctx: &mut StrategyContext,
    ) {
        // 1. Fetch Spot Balances
        match info_client.user_token_balances(user_address).await {
            Ok(balances) => {
                for balance in balances.balances {
                    let total: f64 = balance.total.parse().unwrap_or(0.0);
                    // Assuming 'hold' field exists to calculate available.
                    // If SDK uses different name, this will fail compilation and we will fix.
                    let hold: f64 = balance.hold.parse().unwrap_or(0.0);
                    let available = total - hold;

                    ctx.update_spot_balance(balance.coin, total, available);
                }
            }
            Err(e) => error!("Periodic: Failed to fetch spot balances: {}", e),
        }
        // 2. Fetch Perp Balances (for USDC margin)
        match info_client.user_state(user_address).await {
            Ok(user_state) => {
                let available = user_state.withdrawable.parse().unwrap_or(0.0);
                let total = user_state
                    .margin_summary
                    .account_value
                    .parse()
                    .unwrap_or(0.0);

                ctx.update_perp_balance("USDC".to_string(), total, available);
            }
            Err(e) => error!("Periodic: Failed to fetch perp balances (USDC): {}", e),
        }
    }

    pub async fn run(&self, mut strategy: Box<dyn Strategy>) -> Result<()> {
        info!("Engine started for {}.", self.config.symbol());

        let private_key = &self.exchange_config.private_key;
        let wallet: LocalWallet = private_key
            .parse()
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;
        let user_address = wallet.address();

        // 1. Setup Clients
        let mut info_client = self.setup_info_client().await?;
        let exchange_client = self.setup_exchange_client(wallet.clone()).await?;

        // 2. Load Metadata
        let markets = self.load_metadata(&mut info_client).await?;

        let target_symbol = self.config.symbol();
        if !markets.contains_key(target_symbol) {
            return Err(anyhow!(
                "Critical Error: Metadata for symbol '{}' not found. Please check your configuration.",
                target_symbol
            ));
        } else {
            info!("Metadata loaded for {}.", target_symbol);
        }

        // 3. Init State
        let mut ctx = StrategyContext::new(markets);

        // 4. Initial Balances
        info!("Fetching initial balances...");
        self.fetch_balances(&mut info_client, user_address, &mut ctx)
            .await;

        self.log_balances(&ctx);

        // 5. Setup Leverage/Margin for Perp strategies
        if let StrategyConfig::PerpGrid {
            leverage,
            is_isolated,
            ..
        } = &self.config
        {
            let is_cross = !is_isolated; // is_cross = true means cross margin
            let margin_mode = if is_cross { "Cross" } else { "Isolated" };
            info!(
                "Setting up {} margin with {}x leverage for {}...",
                margin_mode, leverage, target_symbol
            );

            match exchange_client
                .update_leverage(*leverage, target_symbol, is_cross, None)
                .await
            {
                Ok(response) => {
                    info!(
                        "Leverage updated: {}x {} margin for {} - {:?}",
                        leverage, margin_mode, target_symbol, response
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to update leverage for {}: {}. Continuing with existing settings.",
                        target_symbol, e
                    );
                }
            }
        }

        // 5. Subscribe
        let market_info = ctx.market_info(target_symbol).unwrap();
        let string_coin = market_info.coin.clone();

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

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

        let mut runtime = EngineRuntime::new(ctx);
        let mut balance_refresh_timer = tokio::time::interval(std::time::Duration::from_secs(30));
        let mut status_summary_timer =
            tokio::time::interval(std::time::Duration::from_millis(1000)); // 1Hz status update

        // Broadcast Config
        let config_json = serde_json::to_value(&self.config).unwrap_or(serde_json::Value::Null);
        self.broadcaster.send(WSEvent::Config(config_json));

        info!("Starting Event Loop...");
        loop {
            tokio::select! {
                 _ = balance_refresh_timer.tick() => {
                    self.fetch_balances(&mut info_client, user_address, &mut runtime.ctx).await;
                 }
                 _ = status_summary_timer.tick() => {
                    // Periodic Summary Broadcast
                    let summary = strategy.get_summary(&runtime.ctx);
                    match summary {
                        StrategySummary::SpotGrid(s) => {
                            self.broadcaster.send(WSEvent::SpotGridSummary(s));
                        }
                        StrategySummary::PerpGrid(s) => {
                            self.broadcaster.send(WSEvent::PerpGridSummary(s));
                        }
                    }

                    // Also broadcast grid state periodically (ensures cache is populated)
                    let grid_state = strategy.get_grid_state(&runtime.ctx);
                    self.broadcaster.send(WSEvent::GridState(grid_state));
                 }
                 _ = tokio::signal::ctrl_c() => {
                    info!("Shutdown signal received. Stopping Engine...");
                    break;
                 }
                 Some(message) = receiver.recv() => {
                     self.handle_message(message, &mut runtime, &mut strategy, &exchange_client, &string_coin).await?;
                 }
            }
        }
        info!("Engine stopped gracefully.");
        Ok(())
    }

    fn log_balances(&self, ctx: &StrategyContext) {
        info!("========================================");
        info!("           BALANCE SNAPSHOT             ");
        info!("========================================");

        info!("--- Spot Market ---");
        let mut spot_assets: Vec<_> = ctx.spot_balances.keys().collect();
        spot_assets.sort();

        if spot_assets.is_empty() {
            info!("(No Spot Balances)");
        } else {
            for asset in spot_assets {
                if let Some(balance) = ctx.spot_balances.get(asset) {
                    if balance.total > 0.0 {
                        info!(
                            "{:<10} | Total: {:<12.4} | Avail: {:<12.4}",
                            asset, balance.total, balance.available
                        );
                    }
                }
            }
        }

        info!("");
        info!("--- Perp Market ---");
        let mut perp_assets: Vec<_> = ctx.perp_balances.keys().collect();
        perp_assets.sort();

        if perp_assets.is_empty() {
            info!("(No Perp Balances)");
        } else {
            for asset in perp_assets {
                if let Some(balance) = ctx.perp_balances.get(asset) {
                    if balance.total > 0.0 {
                        info!(
                            "{:<10} | Total: {:<12.4} | Avail: {:<12.4}",
                            asset, balance.total, balance.available
                        );
                    }
                }
            }
        }
        info!("========================================");
    }

    async fn handle_message(
        &self,
        message: hyperliquid_rust_sdk::Message,
        runtime: &mut EngineRuntime,
        strategy: &mut Box<dyn Strategy>,
        exchange_client: &ExchangeClient,
        coin: &str,
    ) -> Result<()> {
        match message {
            hyperliquid_rust_sdk::Message::AllMids(all_mids) => {
                if let Some(price_str) = all_mids.data.mids.get(coin) {
                    let mid_price = price_str.parse::<f64>().unwrap_or(0.0);
                    if mid_price > 0.0 {
                        self.process_tick(mid_price, runtime, strategy, exchange_client, coin)
                            .await?;
                    }
                }
            }
            hyperliquid_rust_sdk::Message::User(user_events) => {
                self.process_user_events(user_events.data, runtime, strategy, coin)
                    .await;
            }
            _ => {}
        }
        Ok(())
    }

    async fn process_tick(
        &self,
        mid_price: f64,
        runtime: &mut EngineRuntime,
        strategy: &mut Box<dyn Strategy>,
        exchange_client: &ExchangeClient,
        coin: &str,
    ) -> Result<()> {
        let target_symbol = self.config.symbol();
        // Update Market Info
        if let Some(info) = runtime.ctx.market_info_mut(target_symbol) {
            info.last_price = mid_price;
        }

        // Broadcast Market Update (Real-time)
        // Optimization: Could throttle this if it's too much data, but mid_price updates are usually manageable
        self.broadcaster
            .send(WSEvent::MarketUpdate(MarketEvent { price: mid_price }));

        // Call Strategy
        strategy.on_tick(mid_price, &mut runtime.ctx)?;

        // Execute Order Queue
        while let Some(order_req) = runtime.ctx.order_queue.pop() {
            self.process_order_request(
                order_req,
                runtime,
                strategy,
                exchange_client,
                coin,
                mid_price,
            )
            .await;
        }

        // Execute Cancellation Queue
        while let Some(cloid_to_cancel) = runtime.ctx.cancellation_queue.pop() {
            info!(
                "Processing Cancellation Request for cloid: {}",
                cloid_to_cancel
            );

            // Broadcast Order Update (Cancel Sent)
            self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                oid: 0,
                cloid: Some(cloid_to_cancel.to_string()),
                side: "UNKNOWN".to_string(), // We don't track side for cancels effectively here without lookup
                price: 0.0,
                size: 0.0,
                status: "CANCELLING".to_string(),
                fee: 0.0,
            }));

            let cancel_req = hyperliquid_rust_sdk::ClientCancelRequestCloid {
                asset: coin.to_string(),
                cloid: cloid_to_cancel.as_uuid(),
            };
            match exchange_client.cancel_by_cloid(cancel_req, None).await {
                Ok(res) => info!("Cancel successful: {:?}", res),
                Err(e) => error!("Failed to cancel order {}: {:?}", cloid_to_cancel, e),
            }
        }

        Ok(())
    }

    async fn process_order_request(
        &self,
        order_req: crate::model::OrderRequest,
        runtime: &mut EngineRuntime,
        strategy: &mut Box<dyn Strategy>,
        exchange_client: &ExchangeClient,
        coin: &str,
        mid_price: f64,
    ) {
        let req_summary = match &order_req {
            crate::model::OrderRequest::Limit {
                symbol,
                side,
                price,
                sz,
                reduce_only,
                ..
            } => {
                format!(
                    "LIMIT {} {} {} @ {}{}",
                    side,
                    sz,
                    symbol,
                    price,
                    if *reduce_only { " (RO)" } else { "" }
                )
            }
            crate::model::OrderRequest::Market {
                symbol, side, sz, ..
            } => {
                format!("MARKET {} {} {}", side, sz, symbol)
            }
            crate::model::OrderRequest::Cancel { cloid } => format!("CANCEL {}", cloid),
        };
        info!("[ORDER_PROCESSING] {}", req_summary);

        match order_req {
            crate::model::OrderRequest::Cancel { cloid } => {
                self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                    oid: 0,
                    cloid: Some(cloid.to_string()),
                    side: "UNKNOWN".to_string(),
                    price: 0.0,
                    size: 0.0,
                    status: "CANCELLING".to_string(),
                    fee: 0.0,
                }));
                // Handled via separate cancellation logic usually, but keep fallback
                info!(
                    "Processing Cancel variant in Order Queue for cloid: {}",
                    cloid
                );
                let cancel_req = hyperliquid_rust_sdk::ClientCancelRequestCloid {
                    asset: coin.to_string(),
                    cloid: cloid.as_uuid(),
                };
                match exchange_client.cancel_by_cloid(cancel_req, None).await {
                    Ok(res) => info!("Cancel successful: {:?}", res),
                    Err(e) => error!("Failed to cancel order {}: {:?}", cloid, e),
                }
                return;
            }
            _ => {}
        }

        let target_symbol = self.config.symbol();
        let (side, limit_px, sz, reduce_only, order_type, cloid, target_sz) = match order_req {
            crate::model::OrderRequest::Limit {
                symbol: _,
                side,
                price,
                sz,
                reduce_only,
                cloid,
            } => (
                side,
                price,
                sz,
                reduce_only,
                ClientOrder::Limit(ClientLimit {
                    tif: "Gtc".to_string(),
                }),
                cloid,
                sz,
            ),
            crate::model::OrderRequest::Market {
                symbol: _,
                side,
                sz,
                cloid,
            } => {
                let aggressive_price = if side.is_buy() {
                    mid_price * 1.1
                } else {
                    mid_price * 0.9
                };
                let market_info = runtime.ctx.market_info(target_symbol).unwrap();
                let rounded_aggressive_price = market_info.round_price(aggressive_price);

                (
                    side,
                    rounded_aggressive_price,
                    sz,
                    false,
                    ClientOrder::Limit(ClientLimit {
                        tif: "Ioc".to_string(),
                    }),
                    cloid,
                    sz,
                )
            }
            _ => unreachable!("Cancel already handled"),
        };

        let sdk_req = ClientOrderRequest {
            asset: coin.to_string(),
            is_buy: side.is_buy(),
            limit_px,
            sz,
            reduce_only,
            order_type,
            cloid: cloid.map(|c| c.as_uuid()),
        };

        // Broadcast Order Placed (OPEN via Request)
        if let Some(c) = cloid {
            self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                oid: 0, // Not assigned yet
                cloid: Some(c.to_string()),
                side: side.to_string(),
                price: limit_px,
                size: sz,
                status: "OPENING".to_string(),
                fee: 0.0,
            }));
        }

        // Audit Log: REQ
        if let Some(logger) = &self.audit_logger {
            logger.log_req(
                target_symbol,
                &side.to_string(),
                limit_px,
                sz,
                reduce_only,
                cloid.map(|c| c.to_string()),
            );
        }

        info!(
            "[ORDER_SENT] {} -> Exchange ({})",
            cloid
                .map(|c| c.to_string())
                .unwrap_or_else(|| "no-cloid".to_string()),
            req_summary
        );
        match exchange_client.order(sdk_req, None).await {
            Ok(res) => {
                let mut immediate_fill = None;
                let status_msg = match res {
                    hyperliquid_rust_sdk::ExchangeResponseStatus::Ok(exchange_res) => {
                        if let Some(data) = &exchange_res.data {
                            data.statuses
                                .iter()
                                .map(|s| match s {
                                    hyperliquid_rust_sdk::ExchangeDataStatus::Resting(r) => {
                                        format!("Resting (oid: {})", r.oid)
                                    }
                                    hyperliquid_rust_sdk::ExchangeDataStatus::Filled(f) => {
                                        immediate_fill =
                                            Some((f.total_sz.clone(), f.avg_px.clone()));
                                        format!("Filled (oid: {})", f.oid)
                                    }
                                    hyperliquid_rust_sdk::ExchangeDataStatus::Error(e) => {
                                        format!("Error: {}", e)
                                    }
                                    _ => format!("{:?}", s),
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
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
                        info!("Immediate Fill detected for {}. Notifying strategy.", c);

                        // Broadcast Filled
                        self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                            oid: 0,
                            cloid: Some(c.to_string()),
                            side: side.to_string(),
                            price: px,
                            size: amount,
                            status: "FILLED".to_string(),
                            fee: 0.0, // Fee not always available immediately in response
                        }));

                        if let Err(e) = strategy.on_order_filled(
                            &OrderFill {
                                side,
                                size: amount,
                                price: px,
                                fee: 0.0,
                                cloid: Some(c),
                                reduce_only: Some(reduce_only),
                                raw_dir: None, // Immediate fill from order response, no dir available
                            },
                            &mut runtime.ctx,
                        ) {
                            error!("Strategy on_order_filled error: {}", e);
                        } else {
                            // Broadcast grid state after fill
                            let grid_state = strategy.get_grid_state(&runtime.ctx);
                            self.broadcaster.send(WSEvent::GridState(grid_state));
                        }
                        runtime.completed_cloids.insert(c);
                    }
                } else {
                    if let Some(c) = cloid {
                        runtime.pending_orders.insert(
                            c,
                            PendingOrder {
                                target_size: target_sz,
                                filled_size: 0.0,
                                weighted_avg_px: 0.0,
                                accumulated_fees: 0.0,
                                reduce_only,
                            },
                        );

                        // Broadcast Placing/Resting confirmed
                        self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                            oid: 0,
                            cloid: Some(c.to_string()),
                            side: side.to_string(),
                            price: limit_px,
                            size: target_sz,
                            status: "OPEN".to_string(),
                            fee: 0.0,
                        }));
                    }
                }
            }
            Err(e) => {
                error!("Failed to place order: {:?}", e);
                if let Some(c) = cloid {
                    self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                        oid: 0,
                        cloid: Some(c.to_string()),
                        side: "UNKNOWN".to_string(),
                        price: 0.0,
                        size: 0.0,
                        status: "FAILED".to_string(),
                        fee: 0.0,
                    }));
                    if let Err(strategy_err) = strategy.on_order_failed(c, &mut runtime.ctx) {
                        error!("Strategy on_order_failed error: {}", strategy_err);
                    }
                }
            }
        }
    }

    async fn process_user_events(
        &self,
        user_events_data: UserData,
        runtime: &mut EngineRuntime,
        strategy: &mut Box<dyn Strategy>,
        coin: &str,
    ) {
        let event_type = match &user_events_data {
            UserData::Fills(f) => format!("Fills ({})", f.len()),
            UserData::Funding(_) => "Funding".to_string(),
            UserData::Liquidation(_) => "Liquidation".to_string(),
            UserData::NonUserCancel(c) => format!("NonUserCancel ({})", c.len()),
        };
        debug!("User Event: {}", event_type);

        if let UserData::Fills(fills) = user_events_data {
            for fill in fills {
                if fill.coin != coin {
                    debug!(
                        "Ignoring fill for different coin: {} (expected: {})",
                        fill.coin, coin
                    );
                    continue;
                }

                let amount: f64 = fill.sz.parse().unwrap_or(0.0);
                let px: f64 = fill.px.parse().unwrap_or(0.0);

                // Parse cloid from fill using Cloid::from_hex_str
                let cloid: Option<Cloid> = fill.cloid.as_ref().and_then(|s| Cloid::from_hex_str(s));

                // Debug: Log raw fill fields to understand side vs dir semantics
                // - side: Order book side the order was on ("A" = Ask/Sell, "B" = Bid/Buy)
                // - dir: Trade direction / position impact ("Open Long", "Close Long", "Open Short", "Close Short", or "Buy"/"Sell" for spot)
                // - start_position: Position size before this fill
                debug!(
                    "[FILL_DEBUG] coin={} | side='{}' | dir='{}' | start_position='{}' | sz={} | px={} | cloid={:?}",
                    fill.coin, fill.side, fill.dir, fill.start_position, amount, px, cloid
                );

                // Determine OrderSide from side field:
                // - 'A' (Ask) = Sell order filled
                // - 'B' (Bid) = Buy order filled
                // The raw_dir is passed through for strategies that need perp-specific context
                let side = if fill.side.to_uppercase().starts_with('B') {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                };
                debug!(
                    "[FILL_DEBUG] Parsed side='{}' -> OrderSide::{}, raw_dir='{}'",
                    fill.side, side, fill.dir
                );
                let fee: f64 = fill.fee.parse().unwrap_or(0.0);

                // Audit Log: FILL
                if let Some(logger) = &self.audit_logger {
                    let record_reduce_only = cloid
                        .and_then(|c| runtime.pending_orders.get(&c))
                        .map(|p| p.reduce_only)
                        .unwrap_or(false);

                    let display_symbol = runtime
                        .ctx
                        .market_info(coin)
                        .map(|m| m.symbol.as_str())
                        .unwrap_or(coin);

                    logger.log_fill(
                        display_symbol,
                        &side.to_string(),
                        px,
                        amount,
                        record_reduce_only,
                        cloid.map(|c| c.to_string()),
                        fee,
                    );
                }

                info!(
                    "[ORDER_FILL] {} {} {} @ {} (Fee: {})",
                    cloid
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "no-cloid".to_string()),
                    side,
                    amount,
                    px,
                    fee
                );

                // Broadcast Fill Event
                self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                    oid: fill.oid,
                    cloid: cloid.map(|c| c.to_string()),
                    side: side.to_string(),
                    price: px,
                    size: amount,
                    status: "FILLED".to_string(),
                    fee,
                }));

                if let Some(c) = cloid {
                    if runtime.completed_cloids.contains(&c) {
                        debug!("Ignored duplicate fill for completed cloid: {}", c);
                        continue;
                    }

                    if let Some(pending) = runtime.pending_orders.get_mut(&c) {
                        let new_total_size = pending.filled_size + amount;
                        pending.weighted_avg_px = (pending.weighted_avg_px * pending.filled_size
                            + px * amount)
                            / new_total_size;
                        pending.filled_size = new_total_size;
                        pending.accumulated_fees += fee;

                        info!(
                            "Order progress for {}: {}/{} filled at avg px {}",
                            c, pending.filled_size, pending.target_size, pending.weighted_avg_px
                        );

                        if pending.filled_size >= pending.target_size * 0.9999 {
                            info!("Order {} fully filled. Notifying strategy.", c);
                            let final_px = pending.weighted_avg_px;
                            let final_sz = pending.filled_size;
                            let final_fee = pending.accumulated_fees;
                            let pending_reduce_only = pending.reduce_only;
                            runtime.pending_orders.remove(&c);

                            if let Err(e) = strategy.on_order_filled(
                                &OrderFill {
                                    side,
                                    size: final_sz,
                                    price: final_px,
                                    fee: final_fee,
                                    cloid: Some(c),
                                    reduce_only: Some(pending_reduce_only),
                                    raw_dir: Some(fill.dir.clone()),
                                },
                                &mut runtime.ctx,
                            ) {
                                error!("Strategy on_order_filled error: {}", e);
                            } else {
                                // Broadcast grid state after fill
                                let grid_state = strategy.get_grid_state(&runtime.ctx);
                                self.broadcaster.send(WSEvent::GridState(grid_state));
                            }
                        }
                    } else {
                        info!("Fill for untracked cloid {}. Forwarding immediately.", c);
                        if let Err(e) = strategy.on_order_filled(
                            &OrderFill {
                                side,
                                size: amount,
                                price: px,
                                fee,
                                cloid: Some(c),
                                reduce_only: None, // Unknown for untracked orders
                                raw_dir: Some(fill.dir.clone()),
                            },
                            &mut runtime.ctx,
                        ) {
                            error!("Strategy on_order_filled error: {}", e);
                        } else {
                            // Broadcast grid state after fill
                            let grid_state = strategy.get_grid_state(&runtime.ctx);
                            self.broadcaster.send(WSEvent::GridState(grid_state));
                        }
                    }
                } else {
                    if let Err(e) = strategy.on_order_filled(
                        &OrderFill {
                            side,
                            size: amount,
                            price: px,
                            fee,
                            cloid: None,
                            reduce_only: None, // Unknown without cloid
                            raw_dir: Some(fill.dir.clone()),
                        },
                        &mut runtime.ctx,
                    ) {
                        error!("Strategy on_order_filled error: {}", e);
                    } else {
                        // Broadcast grid state after fill
                        let grid_state = strategy.get_grid_state(&runtime.ctx);
                        self.broadcaster.send(WSEvent::GridState(grid_state));
                    }
                }
            }
        }
    }
}
