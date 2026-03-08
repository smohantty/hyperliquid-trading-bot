//! Simulation engine for dry-run mode.
//!
//! This engine allows running a strategy in simulation mode to preview
//! what orders would be placed without executing real trades.

use crate::config::exchange::ExchangeConfig;
use crate::config::simulation::SimulationConfig;
use crate::config::strategy::StrategyConfig;
use crate::engine::common;
use crate::engine::context::{MarketInfo, StrategyContext};
use crate::model::OrderRequest;
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use ethers::types::H160;
use hyperliquid_rust_sdk::InfoClient;
use std::collections::HashMap;
use std::str::FromStr;
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
    /// Sets up API client, loads market metadata, fetches real balances,
    /// and then applies any configured balance overrides.
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

        // 4. Initialize real balances, then apply overrides if present.
        info!("[SIMULATION] Fetching real balances from exchange");
        self.fetch_balances(&mut info_client, &mut ctx).await;

        if self.sim_config.balances.is_empty() {
            info!("[SIMULATION] No simulation balance patches configured");
        } else {
            info!("[SIMULATION] Applying configured simulation balances");
            self.apply_simulation_balances(&mut ctx);
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

        if let Some(ctx) = &mut self.ctx {
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
        common::setup_info_client(&self.exchange_config.network).await
    }

    async fn load_metadata(
        &self,
        info_client: &mut InfoClient,
    ) -> Result<HashMap<String, MarketInfo>> {
        common::load_metadata(info_client, "[SIMULATION] ").await
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
        let user_address = match H160::from_str(self.exchange_config.trading_account_address()) {
            Ok(addr) => addr,
            Err(e) => {
                error!("[SIMULATION] Invalid account address: {}", e);
                return;
            }
        };
        common::fetch_balances(info_client, user_address, ctx, "[SIMULATION] ").await;
    }

    fn apply_simulation_balances(&self, ctx: &mut StrategyContext) {
        for (asset, balance) in &self.sim_config.balances {
            ctx.update_spot_balance(asset.clone(), *balance, *balance);
            info!("[SIMULATION] Simulation balance: {}={}", asset, balance);

            if asset.to_uppercase() == "USDC" {
                ctx.update_perp_balance("USDC".to_string(), *balance, *balance);
            }
        }
    }
}
