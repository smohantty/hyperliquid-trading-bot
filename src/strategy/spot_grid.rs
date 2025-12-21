use super::common;
use super::types::{GridType, ZoneState};
use crate::config::strategy::StrategyConfig;
use crate::engine::context::{MarketInfo, StrategyContext};
use crate::model::OrderRequest;
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum StrategyState {
    Initializing,
    WaitingForTrigger,
    AcquiringAssets { cloid: u128 },
    Running,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GridZone {
    index: usize,
    lower_price: f64,
    upper_price: f64,
    size: f64,
    state: ZoneState,
    entry_price: f64,
    order_id: Option<u128>,

    // Performance Metrics
    roundtrip_count: u32,
}

#[allow(dead_code)]
pub struct SpotGridStrategy {
    symbol: String,
    base_asset: String,
    quote_asset: String,
    upper_price: f64,
    lower_price: f64,
    grid_type: GridType,
    grid_count: u32,
    total_investment: f64,
    trigger_price: Option<f64>,

    // Internal State
    zones: Vec<GridZone>,
    active_orders: HashMap<u128, usize>,
    state: StrategyState,
    trade_count: u32,
    start_price: Option<f64>,

    // Strategy Performance
    realized_pnl: f64,
    total_fees: f64,

    // Position Tracking
    inventory: f64,
    avg_entry_price: f64,
}

impl SpotGridStrategy {
    pub fn new(config: StrategyConfig) -> Self {
        match config {
            StrategyConfig::SpotGrid {
                symbol,
                upper_price,
                lower_price,
                grid_type,
                grid_count,
                total_investment,
                trigger_price,
            } => {
                let (base_asset, quote_asset) = match symbol.split_once('/') {
                    Some((b, q)) => (b.to_string(), q.to_string()),
                    None => (symbol.clone(), "USDC".to_string()),
                };

                // Always start in Initializing to allow balance checks in initialize_zones
                Self {
                    symbol,
                    base_asset,
                    quote_asset,
                    upper_price,
                    lower_price,
                    grid_type,
                    grid_count,
                    total_investment,
                    trigger_price,
                    zones: Vec::new(),
                    active_orders: HashMap::new(),
                    state: StrategyState::Initializing,
                    trade_count: 0,
                    start_price: None,
                    realized_pnl: 0.0,
                    total_fees: 0.0,
                    inventory: 0.0,
                    avg_entry_price: 0.0,
                }
            }
            _ => panic!("Invalid config type for SpotGridStrategy"),
        }
    }

    // Removed on_tick from here

    fn initialize_zones(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        if self.grid_count < 2 {
            warn!("[SPOT_GRID] Grid count must be at least 2");
            return Err(anyhow!("Grid count must be at least 2"));
        }

        let market_info = match ctx.market_info(&self.symbol) {
            Some(info) => info.clone(),
            None => {
                error!("[SPOT_GRID] No market info for {}", self.symbol);
                return Err(anyhow!("No market info for {}", self.symbol));
            }
        };

        // 1. Generate Zones
        let total_base_required = self.generate_grid_levels(&market_info);

        info!(
            "[SPOT_GRID] Zones initialized. Total {} Required: {}",
            self.base_asset, total_base_required
        );

        // 2. Check Assets & Acquire if necessary
        self.check_initial_acquisition(ctx, &market_info, total_base_required)
    }

    fn generate_grid_levels(&mut self, market_info: &MarketInfo) -> f64 {
        // Generate Levels
        let prices: Vec<f64> = common::calculate_grid_prices(
            self.grid_type.clone(),
            self.lower_price,
            self.upper_price,
            self.grid_count,
        )
        .into_iter()
        .map(|p| market_info.round_price(p))
        .collect();

        let num_zones = self.grid_count as usize - 1;
        let quote_per_zone = self.total_investment / num_zones as f64;

        // Use trigger_price if available, otherwise last_price
        let initial_price = self.trigger_price.unwrap_or(market_info.last_price);

        self.zones.clear();
        let mut total_base_required = 0.0;

        for i in 0..num_zones {
            let lower = prices[i];
            let upper = prices[i + 1];

            // Calculate size based on quote investment per zone
            let raw_size = quote_per_zone / lower;
            // Enforce minimum order value
            let size = market_info
                .ensure_min_sz(lower, 10.1)
                .max(market_info.round_size(raw_size));

            let initial_state = if initial_price < upper {
                ZoneState::WaitingSell
            } else {
                ZoneState::WaitingBuy
            };

            if initial_state == ZoneState::WaitingSell {
                total_base_required += size;
            }

            self.zones.push(GridZone {
                index: i,
                lower_price: lower,
                upper_price: upper,
                size,
                state: initial_state,
                entry_price: if initial_state == ZoneState::WaitingSell {
                    initial_price
                } else {
                    0.0
                },
                order_id: None,
                roundtrip_count: 0,
            });
        }

        // Normalize total requirement to exchange precision
        market_info.round_size(total_base_required)
    }

    fn check_initial_acquisition(
        &mut self,
        ctx: &mut StrategyContext,
        market_info: &MarketInfo,
        total_base_required: f64,
    ) -> Result<()> {
        let available_base = ctx.get_spot_available(&self.base_asset);
        let deficit = total_base_required - available_base;

        // Use trigger_price if available, otherwise last_price
        let initial_price = self.trigger_price.unwrap_or(market_info.last_price);

        if deficit > market_info.round_size(0.0) {
            // Acquisition Mode
            let mut acquisition_price = initial_price * 0.99; // Fallback default

            if let Some(trigger) = self.trigger_price {
                // If triggered, use trigger price for acquisition
                acquisition_price = market_info.round_price(trigger);
            } else {
                // Standard logic: try to buy at a grid level
                let nearest_level = self
                    .zones
                    .iter()
                    .filter(|z| z.lower_price < market_info.last_price)
                    .map(|z| z.lower_price)
                    .fold(0.0, f64::max);

                if nearest_level > 0.0 {
                    acquisition_price = market_info.round_price(nearest_level);
                } else if !self.zones.is_empty() {
                    acquisition_price = market_info.round_price(self.zones[0].lower_price);
                }
            }

            // Apply 0.2% safety buffer to cover exchange fees
            let deficit_with_buffer = deficit * 1.002;
            let min_acq_sz = market_info.ensure_min_sz(acquisition_price, 10.1);
            let rounded_deficit = min_acq_sz.max(market_info.round_size(deficit_with_buffer));

            if rounded_deficit > 0.0 {
                // Check if we have enough QUOTE (USDC) to buy this deficit
                let estimated_cost = rounded_deficit * acquisition_price;
                let available_quote = ctx.get_spot_available(&self.quote_asset);

                if available_quote < estimated_cost {
                    let msg = format!(
                        "Insufficient Quote Balance for acquisition! Need ~{:.2} {}, Have {:.2} {}. Deficit: {} {}", 
                        estimated_cost, self.quote_asset, available_quote, self.quote_asset, rounded_deficit, self.base_asset
                    );
                    error!("[SPOT_GRID] {}", msg);
                    return Err(anyhow!(msg));
                }

                info!(
                    "[SPOT_GRID] ACQUISITION_NEEDED. Acquiring {} {}. Cost: ~{:.2} {} @ {}.",
                    rounded_deficit,
                    self.base_asset,
                    estimated_cost,
                    self.quote_asset,
                    acquisition_price
                );
                let cloid = ctx.generate_cloid();
                self.state = StrategyState::AcquiringAssets { cloid };

                ctx.place_order(OrderRequest::Limit {
                    symbol: self.symbol.clone(),
                    is_buy: true,
                    price: acquisition_price,
                    sz: rounded_deficit,
                    reduce_only: false,
                    cloid: Some(cloid),
                });
                return Ok(());
            }
        }

        // No Deficit (or negligible)
        if let Some(_trigger) = self.trigger_price {
            // Passive Wait Mode
            info!("[SPOT_GRID] Assets sufficient. Entering WaitingForTrigger state.");
            self.start_price = Some(market_info.last_price);
            self.state = StrategyState::WaitingForTrigger;
        } else {
            // No Trigger, Assets OK -> Running
            info!("[SPOT_GRID] Assets verified. Starting Grid.");
            self.state = StrategyState::Running;
        }

        Ok(())
    }

    fn refresh_orders(&mut self, ctx: &mut StrategyContext) {
        // Collect orders to place to avoid borrowing issues
        let mut orders_to_place = Vec::new();

        for i in 0..self.zones.len() {
            if self.zones[i].order_id.is_none() {
                let zone = &self.zones[i];
                let (price, is_buy) = match zone.state {
                    ZoneState::WaitingBuy => (zone.lower_price, true),
                    ZoneState::WaitingSell => (zone.upper_price, false),
                };

                let cloid = ctx.generate_cloid();
                orders_to_place.push((i, is_buy, price, zone.size, cloid));
            }
        }

        // Execute placement
        for (index, is_buy, price, size, cloid) in orders_to_place {
            let zone = &mut self.zones[index];
            zone.order_id = Some(cloid);
            self.active_orders.insert(cloid, index);

            info!(
                "[SPOT_GRID] Zone {}: Placing {} order at {} (cloid: {})",
                index,
                if is_buy { "BUY" } else { "SELL" },
                price,
                cloid
            );
            ctx.place_order(OrderRequest::Limit {
                symbol: self.symbol.clone(),
                is_buy,
                price,
                sz: size,
                reduce_only: false,
                cloid: Some(cloid),
            });
        }
    }

    fn handle_acquisition_fill(
        &mut self,
        size: f64,
        px: f64,
        fee: f64,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        info!(
            "[SPOT_GRID] Acquisition order filled! Size: {} @ {}. Fee: {}. Starting grid.",
            size, px, fee
        );
        self.total_fees += fee;

        // Acquisition Fill: Initialize/Update Inventory
        // Weighted Average incase we already had some (unlikely but safe)
        let new_inventory = self.inventory + size;
        if new_inventory > 0.0 {
            self.avg_entry_price =
                (self.avg_entry_price * self.inventory + px * size) / new_inventory;
        }
        self.inventory = new_inventory;

        // Update entry_price for all waiting SELL zones to the actual acquisition price
        for zone in &mut self.zones {
            if zone.state == ZoneState::WaitingSell {
                zone.entry_price = px;
            }
        }

        self.state = StrategyState::Running;
        self.refresh_orders(ctx);
        Ok(())
    }

    fn handle_buy_fill(
        &mut self,
        zone_idx: usize,
        size: f64,
        px: f64,
        fee: f64,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        let zone = &mut self.zones[zone_idx];

        info!(
            "[SPOT_GRID] Zone {} | BUY Filled @ {} | Size: {} | Fee: {:.4} | Next: SELL @ {}",
            zone_idx, px, size, fee, zone.upper_price
        );

        // Update Strategy Fees
        self.total_fees += fee;

        // Buy Fill: Increase Inventory & Update Avg Entry Price
        let new_inventory = self.inventory + size;
        if new_inventory > 0.0 {
            // Weighted Average Cost Basis
            self.avg_entry_price =
                (self.avg_entry_price * self.inventory + px * size) / new_inventory;
        }
        self.inventory = new_inventory;

        zone.state = ZoneState::WaitingSell;
        zone.entry_price = px;

        let next_cloid = ctx.generate_cloid();
        let price = zone.upper_price;

        info!(
            "[SPOT_GRID] Zone {} | Placing SELL Order @ {} (cloid: {})",
            zone_idx, price, next_cloid
        );

        self.active_orders.insert(next_cloid, zone_idx);
        zone.order_id = Some(next_cloid);

        ctx.place_order(OrderRequest::Limit {
            symbol: self.symbol.clone(),
            is_buy: false,
            price,
            sz: zone.size,
            reduce_only: false,
            cloid: Some(next_cloid),
        });

        Ok(())
    }

    fn handle_sell_fill(
        &mut self,
        zone_idx: usize,
        size: f64,
        px: f64,
        fee: f64,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        let zone = &mut self.zones[zone_idx];
        let pnl = (px - zone.entry_price) * size;

        // Update Zone Metrics
        zone.roundtrip_count += 1;

        // Update Strategy Metrics
        self.realized_pnl += pnl;
        self.total_fees += fee;

        // Sell Fill: Decrease Inventory
        self.inventory = (self.inventory - size).max(0.0);
        // Avg Entry Price remains unchanged on partial reduction (FIFO/WAC standard)

        info!(
            "[SPOT_GRID] Zone {} | SELL Filled @ {} | Size: {} | PnL: {:.4} | Fee: {:.4} | Next: BUY @ {}",
            zone_idx, px, size, pnl, fee, zone.lower_price
        );

        zone.state = ZoneState::WaitingBuy;
        zone.entry_price = 0.0;

        let next_cloid = ctx.generate_cloid();
        let price = zone.lower_price;

        info!(
            "[SPOT_GRID] Zone {} | Placing BUY Order @ {} (cloid: {})",
            zone_idx, price, next_cloid
        );

        self.active_orders.insert(next_cloid, zone_idx);
        zone.order_id = Some(next_cloid);

        ctx.place_order(OrderRequest::Limit {
            symbol: self.symbol.clone(),
            is_buy: true,
            price,
            sz: zone.size,
            reduce_only: false,
            cloid: Some(next_cloid),
        });

        Ok(())
    }
}

impl Strategy for SpotGridStrategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()> {
        // State Machine
        match self.state {
            StrategyState::Initializing => {
                self.initialize_zones(ctx)?;
            }
            StrategyState::AcquiringAssets { .. } => {
                // Handled in on_order_filled or wait for transition
            }
            StrategyState::WaitingForTrigger => {
                if let Some(trigger) = self.trigger_price {
                    // Directional Trigger Logic
                    // Requires start_price to be set during initialization

                    let start = self
                        .start_price
                        .expect("Start price must be set when in WaitingForTrigger state");

                    if common::check_trigger(price, trigger, start) {
                        info!(
                            "[SPOT_GRID] Price {} crossed trigger {}. Starting.",
                            price, trigger
                        );
                        self.state = StrategyState::Running;
                        self.refresh_orders(ctx);
                    }
                } else {
                    // Should not happen if state is WaitingForTrigger
                    self.state = StrategyState::Running;
                }
            }

            StrategyState::Running => {
                self.refresh_orders(ctx);
            }
        }

        Ok(())
    }

    fn on_order_filled(
        &mut self,
        _side: &str,
        size: f64,
        px: f64,
        fee: f64,
        cloid: Option<u128>,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        if let Some(cloid_val) = cloid {
            // Check for Acquisition Fill
            if let StrategyState::AcquiringAssets { cloid: acq_cloid } = self.state {
                if cloid_val == acq_cloid {
                    return self.handle_acquisition_fill(size, px, fee, ctx);
                }
            }

            if let Some(zone_idx) = self.active_orders.remove(&cloid_val) {
                // Validate Cloid
                {
                    let zone = &self.zones[zone_idx];
                    if zone.order_id != Some(cloid_val) {
                        warn!(
                            "[SPOT_GRID] Zone {} order_id mismatch! Expected {:?}, got {:?}",
                            zone_idx, zone.order_id, cloid_val
                        );
                    }
                }

                // Update Zone State
                self.zones[zone_idx].order_id = None;
                self.trade_count += 1;

                // Clone state to avoid borrow issues while calling helpers
                let state = self.zones[zone_idx].state;

                match state {
                    ZoneState::WaitingBuy => {
                        self.handle_buy_fill(zone_idx, size, px, fee, ctx)?;
                    }
                    ZoneState::WaitingSell => {
                        self.handle_sell_fill(zone_idx, size, px, fee, ctx)?;
                    }
                }
            } else {
                debug!(
                    "[SPOT_GRID] Fill received for unknown/inactive CLOID: {}",
                    cloid_val
                );
            }
        } else {
            debug!("[SPOT_GRID] Fill received without CLOID at price {}", px);
        }

        Ok(())
    }

    fn on_order_failed(&mut self, cloid: u128, _ctx: &mut StrategyContext) -> Result<()> {
        log::warn!("[SPOT_GRID] Order failed callback for cloid: {}", cloid);
        Ok(())
    }

    fn get_status_snapshot(&self, ctx: &StrategyContext) -> crate::broadcast::types::StatusSummary {
        use crate::broadcast::types::{InventoryStats, StatusSummary, WalletStats, ZoneStatus};

        let current_mid = ctx
            .market_info(&self.symbol)
            .map(|m| m.last_price)
            .unwrap_or(0.0);
        let grid_size = self.zones.first().map(|z| z.size).unwrap_or(0.0);

        let refined_zones: Vec<ZoneStatus> = self
            .zones
            .iter()
            .map(|z| {
                let side = if z.lower_price < current_mid {
                    "Buy"
                } else {
                    "Sell"
                };
                let status = if z.order_id.is_some() { "Open" } else { "Idle" };
                ZoneStatus {
                    price: z.lower_price,
                    side: side.to_string(),
                    status: status.to_string(),
                    size: grid_size,
                }
            })
            .collect();

        // Calculate approx unrealized pnl for spot inventory
        let unrealized_pnl = if self.inventory > 0.0 && self.avg_entry_price > 0.0 {
            (current_mid - self.avg_entry_price) * self.inventory
        } else {
            0.0
        };

        // Calculate actual total roundtrips from zones
        let total_roundtrips: u32 = self.zones.iter().map(|z| z.roundtrip_count).sum();

        StatusSummary {
            strategy_name: "SpotGrid".to_string(),
            symbol: self.symbol.clone(),
            realized_pnl: self.realized_pnl,
            unrealized_pnl,
            total_fees: self.total_fees,
            inventory: InventoryStats {
                base_size: self.inventory,
                avg_entry_price: self.avg_entry_price,
            },
            wallet: WalletStats {
                base_balance: ctx.get_spot_total(&self.base_asset),
                quote_balance: ctx.get_spot_total(&self.quote_asset),
            },
            price: current_mid,
            zones: refined_zones,
            custom: serde_json::json!({
                "grid_count": self.zones.len(),
                "range_low": self.lower_price,
                "range_high": self.upper_price,
                "roundtrips": total_roundtrips,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::strategy::StrategyConfig;
    use crate::engine::context::{MarketInfo, StrategyContext};

    #[test]
    fn test_spot_grid_passive_trigger() {
        // Scenario: Assets Sufficient. Wait for trigger.
        // Start Price: 100. Trigger: 105. Expect: Wait until > 105.

        let config = StrategyConfig::SpotGrid {
            symbol: "HYPE/USDC".to_string(),
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            total_investment: 1000.0,
            trigger_price: Some(105.0),
        };

        let mut strategy = SpotGridStrategy::new(config);

        let mut markets = HashMap::new();
        markets.insert(
            "HYPE/USDC".to_string(),
            MarketInfo::new("HYPE/USDC".to_string(), "HYPE".to_string(), 0, 2, 2),
        );
        let mut ctx = StrategyContext::new(markets);
        ctx.update_spot_balance("HYPE".to_string(), 100.0, 100.0); // Sufficient assets (Available = 100)

        if let Some(info) = ctx.market_info_mut("HYPE/USDC") {
            info.last_price = 100.0;
        }

        // Tick 1: Initialization. Should transition to Initializing -> WaitingForTrigger.
        // Strategy starts in Initializing.
        strategy.on_tick(100.0, &mut ctx).unwrap();

        // initialize_zones runs. Finds Assets OK. Trigger Set. -> WaitingForTrigger.
        match strategy.state {
            StrategyState::WaitingForTrigger => (),
            _ => panic!("Expected WaitingForTrigger, got {:?}", strategy.state),
        }
        assert_eq!(strategy.start_price, Some(100.0));

        // Tick 2: Price increase but below trigger
        strategy.on_tick(104.0, &mut ctx).unwrap();
        match strategy.state {
            StrategyState::WaitingForTrigger => (),
            _ => panic!("Expected WaitingForTrigger, got {:?}", strategy.state),
        }

        // Tick 3: Price crosses trigger (105.1)
        strategy.on_tick(105.1, &mut ctx).unwrap();
        match strategy.state {
            StrategyState::Running => (),
            _ => panic!("Expected Running, got {:?}", strategy.state),
        }
    }

    #[test]
    fn test_spot_grid_acquisition_trigger() {
        // Scenario: Low Assets. Trigger defined.
        // Expect: Immediate buy order @ TriggerPrice. State -> AcquiringAssets.

        let config = StrategyConfig::SpotGrid {
            symbol: "HYPE/USDC".to_string(),
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            total_investment: 1000.0,
            trigger_price: Some(105.0),
        };

        let mut strategy = SpotGridStrategy::new(config);

        let mut markets = HashMap::new();
        markets.insert(
            "HYPE/USDC".to_string(),
            MarketInfo::new("HYPE/USDC".to_string(), "HYPE".to_string(), 0, 2, 2),
        );
        let mut ctx = StrategyContext::new(markets);
        ctx.update_spot_balance("HYPE".to_string(), 0.0, 0.0); // Zero assets
        ctx.update_spot_balance("USDC".to_string(), 2000.0, 2000.0); // Sufficient Quote

        if let Some(info) = ctx.market_info_mut("HYPE/USDC") {
            info.last_price = 100.0;
        }

        // Tick 1: Initialization -> Acquisition
        strategy.on_tick(100.0, &mut ctx).unwrap();

        match strategy.state {
            StrategyState::AcquiringAssets { .. } => (),
            _ => panic!("Expected AcquiringAssets, got {:?}", strategy.state),
        }

        // Check placed order
        assert_eq!(ctx.order_queue.len(), 1);
        match &ctx.order_queue[0] {
            crate::model::OrderRequest::Limit { price, .. } => {
                assert_eq!(*price, 105.0); // Should be trigger price
            }
            _ => panic!("Expected Limit order"),
        }

        // Simulate Fill for Acquisition Order
        let fill_price = 104.5;
        let fill_size = 10.0; // Needs to cover deficit
        let fee = 0.05;
        let acq_cloid = match strategy.state {
            StrategyState::AcquiringAssets { cloid } => cloid,
            _ => panic!("Lost AcquiringAssets state"),
        };

        strategy
            .on_order_filled("B", fill_size, fill_price, fee, Some(acq_cloid), &mut ctx)
            .unwrap();

        // Verify that WaitingSell zones now have entry_price = fill_price
        for zone in &strategy.zones {
            if zone.state == ZoneState::WaitingSell {
                assert_eq!(
                    zone.entry_price, fill_price,
                    "Zone {} entry price mismatch",
                    zone.index
                );
            }
        }
    }
    #[test]
    fn test_spot_grid_performance_tracking() {
        // Scenario: verify PnL/Fee/Roundtrip tracking
        // 1. Start strategy
        // 2. Fill Buy Order (Check Fee)
        // 3. Fill Sell Order (Check PnL, Fee, Roundtrip)

        let config = StrategyConfig::SpotGrid {
            symbol: "HYPE/USDC".to_string(),
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            total_investment: 1000.0,
            trigger_price: None,
        };

        let mut strategy = SpotGridStrategy::new(config);
        let mut markets = HashMap::new();
        markets.insert(
            "HYPE/USDC".to_string(),
            MarketInfo::new("HYPE/USDC".to_string(), "HYPE".to_string(), 0, 2, 2),
        );
        let mut ctx = StrategyContext::new(markets);
        ctx.update_spot_balance("HYPE".to_string(), 100.0, 100.0); // Sufficient

        if let Some(info) = ctx.market_info_mut("HYPE/USDC") {
            info.last_price = 100.0;
        }

        // 1. Initialize
        strategy.on_tick(100.0, &mut ctx).unwrap();
        assert_eq!(strategy.state, StrategyState::Running);

        // Tick again to trigger refresh_orders (Running state logic)
        strategy.on_tick(100.0, &mut ctx).unwrap();

        // Get a zone that is WaitingBuy (Price < Upper)
        // Grid: 90, 95, 100, 105, 110
        // Price 100.
        // Zone 0 (90-95): 100 > 95 -> WaitingBuy
        // Zone 1 (95-100): 100 >= 100 -> WaitingBuy
        // Zone 2 (100-105): 100 < 105 -> WaitingSell (Entry 100)
        // Zone 3 (105-110): 100 < 110 -> WaitingSell (Entry 100)

        // Let's pick Zone 1 (95-100). It should be WaitingBuy.
        // Find the active order for Zone 1.
        let zone_idx = 1;
        let zone = &strategy.zones[zone_idx];
        assert_eq!(zone.state, ZoneState::WaitingBuy);
        let order_id = zone.order_id.expect("Zone 1 should have an order");

        // 2. Fill Buy Order
        // Price: 95.0. Size: zone.size. Fee: 0.1.
        let fill_price = 95.0;
        let fill_size = zone.size;
        let buy_fee = 0.1;

        strategy
            .on_order_filled(
                "B",
                fill_size,
                fill_price,
                buy_fee,
                Some(order_id),
                &mut ctx,
            )
            .unwrap();

        // Verify Buy Metrics
        assert_eq!(strategy.total_fees, buy_fee);
        let zone = &strategy.zones[zone_idx];
        assert_eq!(zone.state, ZoneState::WaitingSell);
        assert_eq!(zone.entry_price, fill_price);

        // Get new Sell Order ID
        let sell_order_id = zone.order_id.expect("Zone 1 should have sell order");

        // 3. Fill Sell Order
        // Price: 100.0. Fee: 0.1.
        let sell_price = 100.0;

        let sell_fee = 0.1;

        strategy
            .on_order_filled(
                "S",
                fill_size,
                sell_price,
                sell_fee,
                Some(sell_order_id),
                &mut ctx,
            )
            .unwrap();

        // Verify Sell Metrics (Cycle Complete)
        // PnL = (100 - 95) * size
        let expected_pnl = (sell_price - fill_price) * fill_size;
        let expected_total_fees = buy_fee + sell_fee;

        assert_eq!(strategy.realized_pnl, expected_pnl);
        assert_eq!(strategy.total_fees, expected_total_fees);

        let zone = &strategy.zones[zone_idx];
        assert_eq!(zone.state, ZoneState::WaitingBuy); // Reset
        assert_eq!(zone.roundtrip_count, 1);
    }

    #[test]
    fn test_spot_grid_position_tracking() {
        // Scenario: verify Inventory and Avg Entry Price tracking
        // 1. Start with 0 Inventory
        // 2. Buy 10 @ 100 -> Inventory 10, Avg 100
        // 3. Buy 10 @ 110 -> Inventory 20, Avg 105
        // 4. Sell 5 @ 120 -> Inventory 15, Avg 105

        let config = StrategyConfig::SpotGrid {
            symbol: "HYPE/USDC".to_string(),
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5,
            total_investment: 1000.0,
            trigger_price: None,
        };

        let mut strategy = SpotGridStrategy::new(config);
        let mut markets = HashMap::new();
        markets.insert(
            "HYPE/USDC".to_string(),
            MarketInfo::new("HYPE/USDC".to_string(), "HYPE".to_string(), 0, 2, 2),
        );
        let mut ctx = StrategyContext::new(markets);
        ctx.update_spot_balance("HYPE".to_string(), 100.0, 100.0);

        if let Some(info) = ctx.market_info_mut("HYPE/USDC") {
            info.last_price = 100.0;
        }

        // 1. Init
        assert_eq!(strategy.inventory, 0.0);
        assert_eq!(strategy.avg_entry_price, 0.0);
        strategy.on_tick(100.0, &mut ctx).unwrap();

        // Check if zones initialized
        if strategy.zones.is_empty() {
            strategy.on_tick(100.0, &mut ctx).unwrap();
        }
        assert!(!strategy.zones.is_empty(), "Zones should be initialized");

        // 2. Buy 10 @ 100
        let zone_idx = 0;
        let zone = &mut strategy.zones[zone_idx];
        zone.state = ZoneState::WaitingBuy;
        let order_id = 123;
        zone.order_id = Some(order_id);
        strategy.active_orders.insert(order_id, zone_idx);

        strategy
            .on_order_filled("B", 10.0, 100.0, 0.1, Some(order_id), &mut ctx)
            .unwrap();

        assert_eq!(strategy.inventory, 10.0);
        assert_eq!(strategy.avg_entry_price, 100.0);

        // 3. Buy 10 @ 110 (Artificial fill to test logic)
        // Reset zone to WaitingBuy for test
        let zone = &mut strategy.zones[zone_idx];
        zone.state = ZoneState::WaitingBuy;
        let order_id_2 = 124;
        zone.order_id = Some(order_id_2);
        strategy.active_orders.insert(order_id_2, zone_idx);

        strategy
            .on_order_filled("B", 10.0, 110.0, 0.1, Some(order_id_2), &mut ctx)
            .unwrap();

        assert_eq!(strategy.inventory, 20.0);
        assert_eq!(strategy.avg_entry_price, 105.0); // (10*100 + 10*110) / 20 = 105

        // 4. Sell 5 @ 120
        let zone = &mut strategy.zones[zone_idx];
        zone.state = ZoneState::WaitingSell; // Force state
        let order_id_3 = 125;
        zone.order_id = Some(order_id_3);
        strategy.active_orders.insert(order_id_3, zone_idx);

        strategy
            .on_order_filled("S", 5.0, 120.0, 0.1, Some(order_id_3), &mut ctx)
            .unwrap();

        assert_eq!(strategy.inventory, 15.0);
        assert_eq!(strategy.avg_entry_price, 105.0); // Stays same on sell
    }
}
