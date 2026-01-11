//! Simulation engine for dry-run mode.
//!
//! This engine allows running a strategy in simulation mode to preview
//! what orders would be placed without executing real trades.

use crate::config::exchange::ExchangeConfig;
use crate::config::simulation::{BalanceMode, SimulationConfig};
use crate::config::strategy::StrategyConfig;
use crate::engine::context::{MarketInfo, StrategyContext};
use crate::model::OrderRequest;
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use hyperliquid_rust_sdk::{BaseUrl, InfoClient};
use std::collections::HashMap;
use tracing::{error, info};

/// Simulation engine for dry-run preview.
///
/// Supports single-step execution to show what orders a strategy
/// would place given the current market state.
pub struct SimulationEngine {
    config: StrategyConfig,
    exchange_config: ExchangeConfig,
    sim_config: SimulationConfig,
    ctx: Option<StrategyContext>,
    markets: HashMap<String, MarketInfo>,
    current_price: f64,
}

impl SimulationEngine {
    /// Create a new simulation engine.
    pub fn new(
        config: StrategyConfig,
        exchange_config: ExchangeConfig,
        sim_config: SimulationConfig,
    ) -> Self {
        Self {
            config,
            exchange_config,
            sim_config,
            ctx: None,
            markets: HashMap::new(),
            current_price: 0.0,
        }
    }

    /// Initialize the simulation engine.
    ///
    /// Sets up API client, loads market metadata, and initializes balances
    /// based on the configured balance mode.
    pub async fn initialize(&mut self) -> Result<()> {
        // 1. Setup Info Client
        let mut info_client = self.setup_info_client().await?;

        // 2. Load Markets
        self.markets = self.load_metadata(&mut info_client).await?;

        let target_symbol = self.config.symbol();
        if !self.markets.contains_key(target_symbol) {
            return Err(anyhow!(
                "Symbol '{}' not found in available markets",
                target_symbol
            ));
        }

        // 3. Create Context
        let mut ctx = StrategyContext::new(self.markets.clone());

        // 4. Initialize Balances Based on Mode
        match self.sim_config.balance_mode {
            BalanceMode::Real => {
                info!("[SIMULATION] Using real balances from exchange");
                self.fetch_balances(&mut info_client, &mut ctx).await;
            }
            BalanceMode::Unlimited => {
                info!("[SIMULATION] Using unlimited balances");
                self.inject_unlimited_balances(&mut ctx);
            }
            BalanceMode::Override => {
                info!("[SIMULATION] Using real balances with overrides");
                self.fetch_balances(&mut info_client, &mut ctx).await;
                self.apply_balance_overrides(&mut ctx);
            }
        }

        self.ctx = Some(ctx);
        Ok(())
    }

    /// Run a single step: fetch current price, execute one on_tick.
    ///
    /// Returns the fetched market price.
    pub async fn run_single_step(&mut self, strategy: &mut Box<dyn Strategy>) -> Result<f64> {
        let mut info_client = self.setup_info_client().await?;

        // Fetch current price
        let price = self.fetch_current_price(&mut info_client).await?;
        if price <= 0.0 {
            return Err(anyhow!("Could not determine current market price"));
        }
        self.current_price = price;

        // Update market info with current price
        let target_symbol = self.config.symbol();
        if let Some(ctx) = &mut self.ctx {
            if let Some(info) = ctx.market_info_mut(target_symbol) {
                info.last_price = price;
            }

            // Run strategy tick
            strategy.on_tick(price, ctx)?;
        }

        Ok(price)
    }

    /// Get strategy summary.
    pub fn get_summary(
        &self,
        strategy: &dyn Strategy,
    ) -> Option<crate::broadcast::types::StrategySummary> {
        self.ctx.as_ref().map(|ctx| strategy.get_summary(ctx))
    }

    /// Get grid state.
    pub fn get_grid_state(
        &self,
        strategy: &dyn Strategy,
    ) -> Option<crate::broadcast::types::GridState> {
        self.ctx.as_ref().map(|ctx| strategy.get_grid_state(ctx))
    }

    /// Get pending orders from the context queue.
    pub fn get_orders(&self) -> Vec<OrderRequest> {
        self.ctx
            .as_ref()
            .map(|ctx| ctx.order_queue.clone())
            .unwrap_or_default()
    }

    /// Get the current market price.
    pub fn get_current_price(&self) -> f64 {
        self.current_price
    }

    /// Get the strategy config.
    pub fn config(&self) -> &StrategyConfig {
        &self.config
    }

    // --- Private Methods ---

    async fn setup_info_client(&self) -> Result<InfoClient> {
        let base_url = if self.exchange_config.network == "mainnet" {
            BaseUrl::Mainnet
        } else {
            BaseUrl::Testnet
        };
        InfoClient::with_reconnect(None, Some(base_url))
            .await
            .map_err(|e| anyhow!("Failed to connect InfoClient: {}", e))
    }

    async fn load_metadata(
        &self,
        info_client: &mut InfoClient,
    ) -> Result<HashMap<String, MarketInfo>> {
        info!("[SIMULATION] Fetching market metadata...");
        let mut markets = HashMap::new();

        // Fetch Spot Metadata
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
            Err(e) => error!("[SIMULATION] Failed to fetch spot metadata: {}", e),
        }

        // Fetch Perp Metadata
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
            Err(e) => error!("[SIMULATION] Failed to fetch perp metadata: {}", e),
        }

        Ok(markets)
    }

    async fn fetch_current_price(&self, info_client: &mut InfoClient) -> Result<f64> {
        let target_symbol = self.config.symbol();
        let market_info = self
            .markets
            .get(target_symbol)
            .ok_or_else(|| anyhow!("Market info not found for {}", target_symbol))?;

        // Fetch L2 orderbook to get mid price
        match info_client.l2_snapshot(market_info.coin.clone()).await {
            Ok(l2) => {
                let best_bid = l2
                    .levels
                    .first()
                    .and_then(|bids| bids.first())
                    .map(|l| l.px.parse::<f64>().unwrap_or(0.0))
                    .unwrap_or(0.0);

                let best_ask = l2
                    .levels
                    .get(1)
                    .and_then(|asks| asks.first())
                    .map(|l| l.px.parse::<f64>().unwrap_or(0.0))
                    .unwrap_or(0.0);

                if best_bid > 0.0 && best_ask > 0.0 {
                    Ok((best_bid + best_ask) / 2.0)
                } else {
                    Err(anyhow!("Could not get valid bid/ask prices"))
                }
            }
            Err(e) => Err(anyhow!("Failed to fetch L2 snapshot: {}", e)),
        }
    }

    async fn fetch_balances(&self, info_client: &mut InfoClient, ctx: &mut StrategyContext) {
        use ethers::types::H160;
        use std::str::FromStr;

        let user_address = match H160::from_str(&self.exchange_config.master_account_address) {
            Ok(addr) => addr,
            Err(e) => {
                error!("[SIMULATION] Invalid account address: {}", e);
                return;
            }
        };

        // Fetch Spot Balances
        match info_client.user_token_balances(user_address).await {
            Ok(balances) => {
                for balance in balances.balances {
                    let total: f64 = balance.total.parse().unwrap_or(0.0);
                    let hold: f64 = balance.hold.parse().unwrap_or(0.0);
                    let available = total - hold;
                    ctx.update_spot_balance(balance.coin, total, available);
                }
            }
            Err(e) => error!("[SIMULATION] Failed to fetch spot balances: {}", e),
        }

        // Fetch Perp Balances
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
            Err(e) => error!("[SIMULATION] Failed to fetch perp balances: {}", e),
        }
    }

    fn inject_unlimited_balances(&self, ctx: &mut StrategyContext) {
        let amount = self.sim_config.unlimited_amount;

        // Parse base/quote from symbol
        let symbol = self.config.symbol();
        let (base_asset, quote_asset) = parse_symbol_assets(symbol);

        // Inject spot balances
        ctx.update_spot_balance(quote_asset.clone(), amount, amount);
        ctx.update_spot_balance(base_asset.clone(), amount, amount);

        // Also update perp margin
        ctx.update_perp_balance("USDC".to_string(), amount, amount);

        info!(
            "[SIMULATION] Injected unlimited balances: {}={}, {}={}, USDC={}",
            base_asset, amount, quote_asset, amount, amount
        );
    }

    fn apply_balance_overrides(&self, ctx: &mut StrategyContext) {
        for (asset, balance) in &self.sim_config.balance_overrides {
            ctx.update_spot_balance(asset.clone(), *balance, *balance);
            info!("[SIMULATION] Override balance: {}={}", asset, balance);

            if asset.to_uppercase() == "USDC" {
                ctx.update_perp_balance("USDC".to_string(), *balance, *balance);
            }
        }
    }
}

/// Parse symbol into (base_asset, quote_asset).
fn parse_symbol_assets(symbol: &str) -> (String, String) {
    if let Some(idx) = symbol.find('/') {
        let base = symbol[..idx].to_string();
        let quote = symbol[idx + 1..].to_string();
        (base, quote)
    } else {
        // Perp symbol - use symbol as base, USDC as quote
        (symbol.to_string(), "USDC".to_string())
    }
}
