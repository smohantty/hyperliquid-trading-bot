//! Live trading engine for real order execution.
//!
//! This engine connects to the exchange via WebSocket, subscribes to market
//! data and user events, and executes orders in real-time.

use crate::broadcast::types::StrategySummary;
use crate::broadcast::{MarketEvent, OrderEvent, StatusBroadcaster, WSEvent};
use crate::config::strategy::StrategyConfig;
use crate::constants::{
    BALANCE_REFRESH_INTERVAL, RECONCILIATION_INTERVAL, STATUS_SUMMARY_INTERVAL,
};
use crate::engine::common;
use crate::engine::context::{MarketInfo, StrategyContext};
use crate::logging::order_audit::OrderAuditLogger;
use crate::model::{Cloid, OrderFill, OrderSide};
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use ethers::signers::LocalWallet;
use ethers::types::H160;
use hyperliquid_rust_sdk::{
    BaseUrl, ClientLimit, ClientOrder, ClientOrderRequest, ExchangeClient, InfoClient, UserData,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use tracing::{debug, error, info, warn};

struct PendingOrder {
    target_size: f64,
    filled_size: f64,
    weighted_avg_px: f64,
    accumulated_fees: f64,
    reduce_only: bool,
    oid: Option<u64>,
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

/// Live trading engine for real order execution.
pub struct Engine {
    config: StrategyConfig,
    exchange_config: crate::config::exchange::ExchangeConfig,
    broadcaster: StatusBroadcaster,
    audit_logger: Option<OrderAuditLogger>,
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
        info!("Connecting to InfoClient...");
        common::setup_info_client(&self.exchange_config.network).await
    }

    async fn setup_exchange_client(&self, wallet: LocalWallet) -> Result<ExchangeClient> {
        let base_url = if self.exchange_config.network == "mainnet" {
            BaseUrl::Mainnet
        } else {
            BaseUrl::Testnet
        };
        info!("Connecting to ExchangeClient...");
        info!(
            "Using Agent Wallet to trade for Account: {}",
            self.exchange_config.master_account_address
        );

        ExchangeClient::new(None, wallet, Some(base_url), None, None)
            .await
            .map_err(|e| anyhow!("Failed to connect ExchangeClient: {}", e))
    }

    async fn load_metadata(
        &self,
        info_client: &mut InfoClient,
    ) -> Result<HashMap<String, MarketInfo>> {
        common::load_metadata(info_client, "").await
    }

    async fn fetch_balances(
        &self,
        info_client: &mut InfoClient,
        user_address: ethers::types::H160,
        ctx: &mut StrategyContext,
    ) {
        common::fetch_balances(info_client, user_address, ctx, "Periodic: ").await;
    }

    pub async fn run(&self, mut strategy: Box<dyn Strategy>) -> Result<()> {
        info!("Engine started for {}.", self.config.symbol());

        let private_key = &self.exchange_config.private_key;
        let wallet: LocalWallet = private_key
            .parse()
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;
        let user_address = H160::from_str(&self.exchange_config.master_account_address)
            .map_err(|e| anyhow!("Invalid account address: {}", e))?;

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
        if let StrategyConfig::PerpGrid(crate::config::strategy::PerpGridConfig {
            leverage,
            is_isolated,
            ..
        }) = &self.config
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

        let mut balance_refresh_timer = tokio::time::interval(BALANCE_REFRESH_INTERVAL);
        let mut status_summary_timer = tokio::time::interval(STATUS_SUMMARY_INTERVAL);
        let mut reconciliation_timer = tokio::time::interval(RECONCILIATION_INTERVAL);

        // Broadcast Config
        let mut config_json = serde_json::to_value(&self.config).unwrap_or(serde_json::Value::Null);
        if let Some(info) = runtime.ctx.market_info(target_symbol) {
            if let Some(obj) = config_json.as_object_mut() {
                obj.insert(
                    "sz_decimals".to_string(),
                    serde_json::json!(info.sz_decimals),
                );
            }
        }
        self.broadcaster.send(WSEvent::Config(config_json));
        self.broadcaster
            .send(WSEvent::Info(crate::broadcast::types::SystemInfo {
                network: self.exchange_config.network.clone(),
                exchange: "hyperliquid".to_string(),
            }));

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
                 _ = reconciliation_timer.tick() => {
                     self.reconcile_orders(&mut info_client, user_address, &mut runtime, &mut strategy).await;
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
        // Broadcast Market Update (Real-time)
        // Optimization: Could throttle this if it's too much data, but mid_price updates are usually manageable
        self.broadcaster
            .send(WSEvent::MarketUpdate(MarketEvent { price: mid_price }));

        // Call Strategy
        strategy.on_tick(mid_price, &mut runtime.ctx)?;

        // Execute Order Queue & Cancellation Queue
        let mut orders_to_place = Vec::new();
        let mut cancels_to_process = Vec::new();

        while let Some(order_req) = runtime.ctx.order_queue.pop() {
            match order_req {
                crate::model::OrderRequest::Cancel { cloid } => {
                    cancels_to_process.push(cloid);
                }
                _ => orders_to_place.push(order_req),
            }
        }

        while let Some(cloid) = runtime.ctx.cancellation_queue.pop() {
            cancels_to_process.push(cloid);
        }

        if !cancels_to_process.is_empty() {
            self.process_bulk_cancels(cancels_to_process, exchange_client, coin)
                .await;
        }

        if !orders_to_place.is_empty() {
            self.process_bulk_orders(
                orders_to_place,
                runtime,
                strategy,
                exchange_client,
                coin,
                mid_price,
            )
            .await;
        }

        Ok(())
    }

    async fn process_bulk_cancels(
        &self,
        cloids: Vec<Cloid>,
        exchange_client: &ExchangeClient,
        coin: &str,
    ) {
        info!("Processing Batch Cancellations: {} orders", cloids.len());

        let mut cancel_reqs = Vec::with_capacity(cloids.len());
        for cloid in &cloids {
            // Broadcast Order Update (Cancel Sent)
            self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                oid: 0,
                cloid: Some(cloid.to_string()),
                side: "UNKNOWN".to_string(),
                price: 0.0,
                size: 0.0,
                status: "CANCELLING".to_string(),
                fee: 0.0,
                is_taker: false,
            }));

            cancel_reqs.push(hyperliquid_rust_sdk::ClientCancelRequestCloid {
                asset: coin.to_string(),
                cloid: cloid.as_uuid(),
            });
        }

        match exchange_client
            .bulk_cancel_by_cloid(cancel_reqs, None)
            .await
        {
            Ok(hyperliquid_rust_sdk::ExchangeResponseStatus::Ok(exchange_res)) => {
                if let Some(data) = &exchange_res.data {
                    for (i, status) in data.statuses.iter().enumerate() {
                        let cloid = cloids.get(i);
                        match status {
                            hyperliquid_rust_sdk::ExchangeDataStatus::Success => {
                                info!("Cancel successful for {:?}", cloid);
                            }
                            hyperliquid_rust_sdk::ExchangeDataStatus::Error(e) => {
                                error!("Failed to cancel order {:?}: {}", cloid, e);
                            }
                            _ => {
                                info!("Cancel status for {:?}: {:?}", cloid, status);
                            }
                        }
                    }
                } else {
                    info!("Bulk cancel response: {:?}", exchange_res);
                }
            }
            Ok(hyperliquid_rust_sdk::ExchangeResponseStatus::Err(e)) => {
                error!("Bulk cancel level error: {}", e);
            }
            Err(e) => {
                error!("Failed to execute bulk cancel: {:?}", e);
            }
        }
    }

    async fn process_bulk_orders(
        &self,
        order_reqs: Vec<crate::model::OrderRequest>,
        runtime: &mut EngineRuntime,
        strategy: &mut Box<dyn Strategy>,
        exchange_client: &ExchangeClient,
        coin: &str,
        mid_price: f64,
    ) {
        info!("[BULK_ORDER] {} orders", order_reqs.len());

        let target_symbol = self.config.symbol();
        let mut sdk_reqs = Vec::with_capacity(order_reqs.len());
        let mut order_contexts = Vec::with_capacity(order_reqs.len());

        for order_req in order_reqs {
            let req_summary = match &order_req {
                crate::model::OrderRequest::Limit {
                    symbol,
                    side,
                    price,
                    sz,
                    reduce_only,
                    ..
                } => format!(
                    "LIMIT {} {} {} @ {}{}",
                    side,
                    sz,
                    symbol,
                    price,
                    if *reduce_only { " (RO)" } else { "" }
                ),
                crate::model::OrderRequest::Market {
                    symbol, side, sz, ..
                } => format!("MARKET {} {} {}", side, sz, symbol),
                _ => continue, // Cancels handled separately
            };

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
                    let market_info = runtime.ctx.market_info(target_symbol).unwrap();
                    let market_price = market_info.round_price(mid_price);

                    (
                        side,
                        market_price,
                        sz,
                        false,
                        ClientOrder::Limit(ClientLimit {
                            tif: "Ioc".to_string(),
                        }),
                        cloid,
                        sz,
                    )
                }
                _ => continue,
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

            info!("[ORDER_SENT] Exchange ({})", req_summary);

            sdk_reqs.push(sdk_req);
            order_contexts.push((cloid, side, target_sz, reduce_only, limit_px));
        }

        if sdk_reqs.is_empty() {
            return;
        }

        match exchange_client.bulk_order(sdk_reqs, None).await {
            Ok(hyperliquid_rust_sdk::ExchangeResponseStatus::Ok(exchange_res)) => {
                if let Some(data) = &exchange_res.data {
                    for (i, status) in data.statuses.iter().enumerate() {
                        let (cloid, side, target_sz, reduce_only, limit_px) = order_contexts[i];

                        match status {
                            hyperliquid_rust_sdk::ExchangeDataStatus::Resting(r) => {
                                if let Some(c) = cloid {
                                    runtime.pending_orders.insert(
                                        c,
                                        PendingOrder {
                                            target_size: target_sz,
                                            filled_size: 0.0,
                                            weighted_avg_px: 0.0,
                                            accumulated_fees: 0.0,
                                            reduce_only,
                                            oid: Some(r.oid),
                                        },
                                    );

                                    // Broadcast Placing/Resting confirmed
                                    self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                                        oid: r.oid,
                                        cloid: Some(c.to_string()),
                                        side: side.to_string(),
                                        price: limit_px,
                                        size: target_sz,
                                        status: "OPEN".to_string(),
                                        fee: 0.0,
                                        is_taker: false,
                                    }));
                                }
                            }
                            hyperliquid_rust_sdk::ExchangeDataStatus::Filled(f) => {
                                let amount: f64 = f.total_sz.parse().unwrap_or(0.0);
                                let px: f64 = f.avg_px.parse().unwrap_or(0.0);
                                info!("[ORDER_FILLED_MARKET] {} {} @ {}", side, amount, px);

                                if let Some(c) = cloid {
                                    // Broadcast Filled
                                    self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                                        oid: f.oid,
                                        cloid: Some(c.to_string()),
                                        side: side.to_string(),
                                        price: px,
                                        size: amount,
                                        status: "FILLED".to_string(),
                                        fee: 0.0,
                                        is_taker: true,
                                    }));

                                    if let Err(e) = strategy.on_order_filled(
                                        &OrderFill {
                                            side,
                                            size: amount,
                                            price: px,
                                            fee: 0.0,
                                            cloid: Some(c),
                                            reduce_only: Some(reduce_only),
                                            raw_dir: None,
                                        },
                                        &mut runtime.ctx,
                                    ) {
                                        error!("Strategy on_order_filled error: {}", e);
                                    } else {
                                        let grid_state = strategy.get_grid_state(&runtime.ctx);
                                        self.broadcaster.send(WSEvent::GridState(grid_state));
                                    }
                                    runtime.completed_cloids.insert(c);
                                }
                            }
                            hyperliquid_rust_sdk::ExchangeDataStatus::Error(e) => {
                                error!("Order Error for {:?}: {}", cloid, e);
                                if let Some(c) = cloid {
                                    self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                                        oid: 0,
                                        cloid: Some(c.to_string()),
                                        side: "UNKNOWN".to_string(),
                                        price: 0.0,
                                        size: 0.0,
                                        status: "FAILED".to_string(),
                                        fee: 0.0,
                                        is_taker: false,
                                    }));
                                    if let Err(strategy_err) =
                                        strategy.on_order_failed(c, &mut runtime.ctx)
                                    {
                                        error!("Strategy on_order_failed error: {}", strategy_err);
                                    }
                                }
                            }
                            _ => {
                                info!("Unknown status for {:?}: {:?}", cloid, status);
                            }
                        }
                    }
                } else {
                    info!("Bulk order raw response: {:?}", exchange_res);
                }
            }
            Ok(hyperliquid_rust_sdk::ExchangeResponseStatus::Err(e)) => {
                error!("Bulk order level error: {}", e);
                // Fail all
                for (cloid, _, _, _, _) in order_contexts {
                    if let Some(c) = cloid {
                        if let Err(strategy_err) = strategy.on_order_failed(c, &mut runtime.ctx) {
                            error!("Strategy on_order_failed error: {}", strategy_err);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to place bulk orders: {:?}", e);
                // Fail all
                for (cloid, _, _, _, _) in order_contexts {
                    if let Some(c) = cloid {
                        if let Err(strategy_err) = strategy.on_order_failed(c, &mut runtime.ctx) {
                            error!("Strategy on_order_failed error: {}", strategy_err);
                        }
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

                // - 'A' (Ask) = Sell order filled
                // - 'B' (Bid) = Buy order filled
                let side = if fill.side.to_uppercase().starts_with('B') {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                };

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

                // Broadcast Fill Event
                self.broadcaster.send(WSEvent::OrderUpdate(OrderEvent {
                    oid: fill.oid,
                    cloid: cloid.map(|c| c.to_string()),
                    side: side.to_string(),
                    price: px,
                    size: amount,
                    status: "FILLED".to_string(),
                    fee,
                    is_taker: false,
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

                        let is_fully_filled = pending.filled_size >= pending.target_size * 0.9999;

                        if is_fully_filled {
                            info!(
                                "[ORDER_FILLED] {} {} @ {} (Fee: {}).",
                                side,
                                pending.filled_size,
                                pending.weighted_avg_px,
                                pending.accumulated_fees
                            );
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
                            runtime.completed_cloids.insert(c);
                        } else {
                            // Partial Fill - Log but don't notify strategy yet (waiting for full fill)
                            info!(
                                "[ORDER_FILL_PARTIAL] {} {} @ {} (Fee: {})",
                                side, amount, px, fee
                            );
                        }
                    } else {
                        info!(
                            "[ORDER_FILL_UNTRACKED] {} {} @ {} (Fee: {})",
                            side, amount, px, fee
                        );
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
                    info!(
                        "[ORDER_FILL_NOCLID] {} {} @ {} (Fee: {})",
                        side, amount, px, fee
                    );
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

    async fn reconcile_orders(
        &self,
        info_client: &mut InfoClient,
        user_address: H160,
        runtime: &mut EngineRuntime,
        strategy: &mut Box<dyn Strategy>,
    ) {
        let open_orders = match info_client.open_orders(user_address).await {
            Ok(orders) => orders,
            Err(e) => {
                error!("Reconciliation: Failed to fetch open orders: {}", e);
                return;
            }
        };

        // Build Set of Exchange OIDs
        let exchange_oids: HashSet<u64> = open_orders.iter().map(|o| o.oid).collect();

        // Check Local Pending Orders
        // Collect to avoid borrow issues
        let pending_entries: Vec<(Cloid, Option<u64>)> = runtime
            .pending_orders
            .iter()
            .map(|(k, v)| (*k, v.oid))
            .collect();

        for (cloid, maybe_oid) in pending_entries {
            if let Some(oid) = maybe_oid {
                // If local pending order (with known OID) is NOT in exchange open orders...
                if !exchange_oids.contains(&oid) {
                    // Check Idempotency (Race Condition Guard)
                    if runtime.completed_cloids.contains(&cloid) {
                        continue;
                    }

                    info!("Reconciliation: Order {} (OID {}) missing from exchange. Querying status...", cloid, oid);

                    // Query Status via REST
                    match info_client.query_order_by_oid(user_address, oid).await {
                        Ok(response) => {
                            if let Some(order_state) = response.order {
                                let status = order_state.status.as_str();
                                if status == "filled" {
                                    let amount: f64 = order_state.order.sz.parse().unwrap_or(0.0);
                                    let px: f64 = order_state.order.limit_px.parse().unwrap_or(0.0);
                                    let side = if order_state.order.side == "B" {
                                        OrderSide::Buy
                                    } else {
                                        OrderSide::Sell
                                    };
                                    let reduce_only = order_state.order.reduce_only;

                                    info!("[RECONCILE_FILLED] {} {} @ {}", side, amount, px);

                                    // Update State
                                    runtime.pending_orders.remove(&cloid);
                                    runtime.completed_cloids.insert(cloid);

                                    if let Err(e) = strategy.on_order_filled(
                                        &OrderFill {
                                            side,
                                            size: amount,
                                            price: px,
                                            fee: 0.0,
                                            cloid: Some(cloid),
                                            reduce_only: Some(reduce_only),
                                            raw_dir: None,
                                        },
                                        &mut runtime.ctx,
                                    ) {
                                        error!("Strategy on_order_filled error (Reconcile): {}", e);
                                    } else {
                                        let grid_state = strategy.get_grid_state(&runtime.ctx);
                                        self.broadcaster.send(WSEvent::GridState(grid_state));
                                    }
                                } else if status == "canceled"
                                    || status == "rejected"
                                    || status == "margin"
                                {
                                    info!("[RECONCILE_FAILED] Order {} was {}", cloid, status);
                                    runtime.pending_orders.remove(&cloid);
                                    runtime.completed_cloids.insert(cloid);
                                    let _ = strategy.on_order_failed(cloid, &mut runtime.ctx);
                                } else {
                                    info!(
                                        "Reconciliation: Order {} status is {}. Waiting.",
                                        cloid, status
                                    );
                                }
                            } else {
                                warn!(
                                    "Reconciliation: Order {} not found by query. Assuming failed.",
                                    cloid
                                );
                                runtime.pending_orders.remove(&cloid);
                                let _ = strategy.on_order_failed(cloid, &mut runtime.ctx);
                            }
                        }
                        Err(e) => {
                            error!(
                                "Reconciliation: Failed to query status for {}: {}",
                                cloid, e
                            );
                        }
                    }
                }
            }
        }
    }
}
