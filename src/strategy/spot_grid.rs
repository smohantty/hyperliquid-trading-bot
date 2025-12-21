use super::common;
use super::types::GridType;
use crate::config::strategy::StrategyConfig;
use crate::engine::context::{MarketInfo, StrategyContext, MIN_NOTIONAL_VALUE};
use crate::model::{Cloid, OrderFill, OrderRequest, OrderSide};
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum StrategyState {
    Initializing,
    WaitingForTrigger,
    AcquiringAssets { cloid: Cloid },
    Running,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GridZone {
    index: usize,
    lower_price: f64,
    upper_price: f64,
    size: f64,
    /// The side of the pending order for this zone (Buy at lower, Sell at upper)
    pending_side: OrderSide,
    entry_price: f64,
    order_id: Option<Cloid>,

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
    active_orders: HashMap<Cloid, usize>,
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
        let (total_base_required, total_quote_required) =
            self.generate_grid_levels(&market_info)?;

        info!(
            "[SPOT_GRID] INITIALIZATION: Asset Required: {} ( {} ), {} ( {} )",
            self.base_asset, total_base_required, self.quote_asset, total_quote_required
        );

        // Seed inventory with what we actually have right now
        // This is critical for accurate tracking once the grid starts
        let available_base = ctx.get_spot_available(&self.base_asset);
        let available_quote = ctx.get_spot_available(&self.quote_asset);
        self.inventory = available_base;

        // Upfront Total Investment Validation
        // Calculate approx market value of our total holdings for this strategy.
        // We use the initial_price (which considers trigger price) as that is the price
        // at which the asset requirements are calculated.
        let initial_price = self.trigger_price.unwrap_or(market_info.last_price);
        let total_wallet_value = (available_base * initial_price) + available_quote;

        if total_wallet_value < self.total_investment {
            let msg = format!(
                "Insufficient Total Portfolio Value! Required: {:.2} {}, Have approx: {:.2} {} ({} {} + {} {}). Bailing out.",
                self.total_investment, self.quote_asset, total_wallet_value, self.quote_asset,
                available_base, self.base_asset, available_quote, self.quote_asset
            );
            error!("[SPOT_GRID] {}", msg);
            return Err(anyhow!(msg));
        }

        // 2. Check Assets & Rebalance if necessary
        self.check_initial_acquisition(ctx, &market_info, total_base_required, total_quote_required)
    }

    fn generate_grid_levels(&mut self, market_info: &MarketInfo) -> Result<(f64, f64)> {
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

        if quote_per_zone < MIN_NOTIONAL_VALUE {
            let msg = format!(
                "Quote per zone ({:.2}) is less than minimum order value ({}). Increase total_investment or decrease grid_count.",
                quote_per_zone, MIN_NOTIONAL_VALUE
            );
            error!("[SPOT_GRID] {}", msg);
            return Err(anyhow!(msg));
        }

        // Use trigger_price if available, otherwise last_price
        let initial_price = self.trigger_price.unwrap_or(market_info.last_price);

        self.zones.clear();
        let mut total_base_required = 0.0;
        let mut total_quote_required = 0.0;

        for i in 0..num_zones {
            let lower = prices[i];
            let upper = prices[i + 1];

            // Calculate size based on quote investment per zone
            let raw_size = quote_per_zone / lower;
            let size = market_info.round_size(raw_size);

            // If price is below upper bound, we already have base asset -> wait to sell
            // Otherwise, we need to buy first
            let pending_side = if initial_price < upper {
                OrderSide::Sell
            } else {
                OrderSide::Buy
            };

            if pending_side.is_sell() {
                total_base_required += size;
            } else {
                total_quote_required += size * lower;
            }

            self.zones.push(GridZone {
                index: i,
                lower_price: lower,
                upper_price: upper,
                size,
                pending_side,
                entry_price: if pending_side.is_sell() {
                    initial_price
                } else {
                    0.0
                },
                order_id: None,
                roundtrip_count: 0,
            });
        }

        // Normalize total requirement to exchange precision
        Ok((
            market_info.round_size(total_base_required),
            total_quote_required,
        ))
    }

    fn check_initial_acquisition(
        &mut self,
        ctx: &mut StrategyContext,
        market_info: &MarketInfo,
        total_base_required: f64,
        total_quote_required: f64,
    ) -> Result<()> {
        let available_base = ctx.get_spot_available(&self.base_asset);
        let available_quote = ctx.get_spot_available(&self.quote_asset);

        let base_deficit = total_base_required - available_base;
        let quote_deficit = total_quote_required - available_quote;

        // Use trigger_price if available, otherwise last_price
        let initial_price = self.trigger_price.unwrap_or(market_info.last_price);

        if base_deficit > 0.0 {
            // Case 1: Not enough base asset (e.g. BTC) to cover the SELL levels.
            // Need to BUY base asset.
            let mut acquisition_price = initial_price;

            if let Some(trigger) = self.trigger_price {
                acquisition_price = market_info.round_price(trigger);
            } else {
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

            let rounded_deficit = market_info.clamp_to_min_notional(
                base_deficit,
                acquisition_price,
                MIN_NOTIONAL_VALUE,
            );

            if rounded_deficit > 0.0 {
                let estimated_cost = rounded_deficit * acquisition_price;

                if available_quote < estimated_cost {
                    let msg = format!(
                        "Insufficient Quote Balance for acquisition! Need ~{:.2} {}, Have {:.2} {}. Base Deficit: {} {}",
                        estimated_cost, self.quote_asset, available_quote, self.quote_asset, rounded_deficit, self.base_asset
                    );
                    error!("[SPOT_GRID] {}", msg);
                    return Err(anyhow!(msg));
                }

                info!(
                    "[ORDER_REQUEST] [SPOT_GRID] REBALANCING: LIMIT BUY {} {} @ {}",
                    rounded_deficit, self.base_asset, acquisition_price
                );
                let cloid = ctx.generate_cloid();
                self.state = StrategyState::AcquiringAssets { cloid };

                ctx.place_order(OrderRequest::Limit {
                    symbol: self.symbol.clone(),
                    side: OrderSide::Buy,
                    price: acquisition_price,
                    sz: rounded_deficit,
                    reduce_only: false,
                    cloid: Some(cloid),
                });
                return Ok(());
            }
        } else if quote_deficit > 0.0 {
            // Case 2: Enough base asset, but NOT enough quote asset (e.g. USDC) for BUY levels.
            // Need to SELL some base asset to get quote.
            let mut acquisition_price = initial_price;

            if let Some(trigger) = self.trigger_price {
                acquisition_price = market_info.round_price(trigger);
            } else {
                // Find nearest level ABOVE market to sell at
                let nearest_sell_level = self
                    .zones
                    .iter()
                    .filter(|z| z.upper_price > market_info.last_price)
                    .map(|z| z.upper_price)
                    .fold(f64::INFINITY, f64::min);

                if nearest_sell_level.is_finite() {
                    acquisition_price = market_info.round_price(nearest_sell_level);
                } else if !self.zones.is_empty() {
                    acquisition_price =
                        market_info.round_price(self.zones.last().unwrap().upper_price);
                }
            }

            let base_to_sell = quote_deficit / acquisition_price;
            let rounded_sell_sz = market_info.clamp_to_min_notional(
                base_to_sell,
                acquisition_price,
                MIN_NOTIONAL_VALUE,
            );

            if rounded_sell_sz > 0.0 {
                let estimated_proceeds = rounded_sell_sz * acquisition_price;

                info!(
                    "[SPOT_GRID] Quote deficit detected: deficit={} {}, need to sell ~{} {} (~${:.2}) @ price {}",
                    quote_deficit, self.quote_asset, rounded_sell_sz, self.base_asset, estimated_proceeds, acquisition_price
                );

                if available_base < rounded_sell_sz {
                    let msg = format!(
                        "Insufficient Base Balance for rebalancing! Need to sell {} {}, Have {} {}. Quote Deficit: {} {}",
                        rounded_sell_sz, self.base_asset, available_base, self.base_asset, quote_deficit, self.quote_asset
                    );
                    error!("[SPOT_GRID] {}", msg);
                    return Err(anyhow!(msg));
                }

                info!(
                    "[ORDER_REQUEST] [SPOT_GRID] REBALANCING: LIMIT SELL {} {} @ {}",
                    rounded_sell_sz, self.base_asset, acquisition_price
                );
                let cloid = ctx.generate_cloid();
                self.state = StrategyState::AcquiringAssets { cloid };

                ctx.place_order(OrderRequest::Limit {
                    symbol: self.symbol.clone(),
                    side: OrderSide::Sell,
                    price: acquisition_price,
                    sz: rounded_sell_sz,
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
        let mut orders_to_place: Vec<(usize, OrderSide, f64, f64, Cloid)> = Vec::new();

        for i in 0..self.zones.len() {
            if self.zones[i].order_id.is_none() {
                let zone = &self.zones[i];
                let price = if zone.pending_side.is_buy() {
                    zone.lower_price
                } else {
                    zone.upper_price
                };

                let cloid = ctx.generate_cloid();
                orders_to_place.push((i, zone.pending_side, price, zone.size, cloid));
            }
        }

        // Execute placement
        for (index, side, price, size, cloid) in orders_to_place {
            let zone = &mut self.zones[index];
            zone.order_id = Some(cloid);
            self.active_orders.insert(cloid, index);

            info!(
                "[ORDER_REQUEST] [SPOT_GRID] GRID_LVL_{}: LIMIT {} {} {} @ {}",
                index, side, size, self.symbol, price
            );
            ctx.place_order(OrderRequest::Limit {
                symbol: self.symbol.clone(),
                side,
                price,
                sz: size,
                reduce_only: false,
                cloid: Some(cloid),
            });
        }
    }

    fn handle_acquisition_fill(
        &mut self,
        fill: &OrderFill,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        info!(
            "[SPOT_GRID] Rebalancing fill received! {} {} @ {}. Fee: {}. Starting grid.",
            if fill.side.is_buy() { "Purchased" } else { "Sold" },
            fill.size,
            fill.price,
            fill.fee
        );
        self.total_fees += fill.fee;

        // Update Inventory
        if fill.side.is_buy() {
            let new_inventory = self.inventory + fill.size;
            if new_inventory > 0.0 {
                self.avg_entry_price =
                    (self.avg_entry_price * self.inventory + fill.price * fill.size) / new_inventory;
            }
            self.inventory = new_inventory;
        } else {
            self.inventory = (self.inventory - fill.size).max(0.0);
            // Avg entry price remains same on sell
        }

        // Update entry_price for all zones waiting to sell to the actual fill price
        // (they now have inventory at this cost basis)
        for zone in &mut self.zones {
            if zone.pending_side.is_sell() {
                zone.entry_price = fill.price;
            }
        }

        self.state = StrategyState::Running;
        self.refresh_orders(ctx);
        Ok(())
    }

    fn handle_buy_fill(
        &mut self,
        zone_idx: usize,
        fill: &OrderFill,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        let next_price = self.zones[zone_idx].upper_price;

        info!(
            "[SPOT_GRID] Zone {} | BUY Filled @ {} | Size: {} | Fee: {:.4} | Next: SELL @ {}",
            zone_idx, fill.price, fill.size, fill.fee, next_price
        );

        // Update Strategy Fees
        self.total_fees += fill.fee;

        // Buy Fill: Increase Inventory & Update Avg Entry Price
        let new_inventory = self.inventory + fill.size;
        if new_inventory > 0.0 {
            // Weighted Average Cost Basis
            self.avg_entry_price =
                (self.avg_entry_price * self.inventory + fill.price * fill.size) / new_inventory;
        }
        self.inventory = new_inventory;

        // Update zone: now waiting to sell
        self.zones[zone_idx].pending_side = OrderSide::Sell;
        self.zones[zone_idx].entry_price = fill.price;

        // Place counter order (Sell at upper price)
        self.place_counter_order(zone_idx, next_price, OrderSide::Sell, ctx)
    }

    fn handle_sell_fill(
        &mut self,
        zone_idx: usize,
        fill: &OrderFill,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        let zone = &self.zones[zone_idx];
        let pnl = (fill.price - zone.entry_price) * fill.size;
        let next_price = zone.lower_price;

        info!(
            "[SPOT_GRID] Zone {} | SELL Filled @ {} | Size: {} | PnL: {:.4} | Fee: {:.4} | Next: BUY @ {}",
            zone_idx, fill.price, fill.size, pnl, fill.fee, next_price
        );

        // Update Zone Metrics
        self.zones[zone_idx].roundtrip_count += 1;

        // Update Strategy Metrics
        self.realized_pnl += pnl;
        self.total_fees += fill.fee;

        // Sell Fill: Decrease Inventory
        self.inventory = (self.inventory - fill.size).max(0.0);
        // Avg Entry Price remains unchanged on partial reduction (FIFO/WAC standard)

        // Update zone: now waiting to buy
        self.zones[zone_idx].pending_side = OrderSide::Buy;
        self.zones[zone_idx].entry_price = 0.0;

        // Place counter order (Buy at lower price)
        self.place_counter_order(zone_idx, next_price, OrderSide::Buy, ctx)
    }

    fn place_counter_order(
        &mut self,
        zone_idx: usize,
        price: f64,
        side: OrderSide,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        let zone = &mut self.zones[zone_idx];
        let next_cloid = ctx.generate_cloid();

        info!(
            "[ORDER_REQUEST] [SPOT_GRID] COUNTER_ORDER: LIMIT {} {} {} @ {}",
            side, zone.size, self.symbol, price
        );

        self.active_orders.insert(next_cloid, zone_idx);
        zone.order_id = Some(next_cloid);

        ctx.place_order(OrderRequest::Limit {
            symbol: self.symbol.clone(),
            side,
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

    fn on_order_filled(&mut self, fill: &OrderFill, ctx: &mut StrategyContext) -> Result<()> {
        if let Some(cloid_val) = fill.cloid {
            // Check for Acquisition Fill
            if let StrategyState::AcquiringAssets { cloid: acq_cloid } = self.state {
                if cloid_val == acq_cloid {
                    return self.handle_acquisition_fill(fill, ctx);
                }
            }

            if let Some(zone_idx) = self.active_orders.remove(&cloid_val) {
                // Validate Cloid
                {
                    let zone = &self.zones[zone_idx];
                    if zone.order_id != Some(cloid_val) {
                        warn!(
                            "[SPOT_GRID] Zone {} order_id mismatch! Expected {:?}, got {}",
                            zone_idx, zone.order_id, cloid_val
                        );
                    }
                }

                // ============================================================
                // FILL VALIDATION ASSERTIONS
                // Verify exchange fill data matches our internal expectations
                // ============================================================
                let expected_side = self.zones[zone_idx].pending_side;

                // 1. Validate fill.side matches zone's pending order side
                if fill.side != expected_side {
                    error!(
                        "[SPOT_GRID] ASSERTION FAILED: Zone {} expected side {:?} but got {:?}",
                        zone_idx, expected_side, fill.side
                    );
                }
                debug_assert_eq!(
                    fill.side, expected_side,
                    "Zone {} fill side mismatch: expected {:?}, got {:?}",
                    zone_idx, expected_side, fill.side
                );

                // 2. Validate raw_dir if present (spot should be "Buy" or "Sell")
                if let Some(ref raw_dir) = fill.raw_dir {
                    let expected_dir = if expected_side.is_buy() { "Buy" } else { "Sell" };
                    if raw_dir != expected_dir {
                        error!(
                            "[SPOT_GRID] ASSERTION FAILED: Zone {} expected raw_dir '{}' but got '{}'",
                            zone_idx, expected_dir, raw_dir
                        );
                    }
                }
                // ============================================================
                // END FILL VALIDATION
                // ============================================================

                // Update Zone State
                self.zones[zone_idx].order_id = None;
                self.trade_count += 1;

                // Route to appropriate fill handler
                if expected_side.is_buy() {
                    self.handle_buy_fill(zone_idx, fill, ctx)?;
                } else {
                    self.handle_sell_fill(zone_idx, fill, ctx)?;
                }
            } else {
                debug!(
                    "[SPOT_GRID] Fill received for unknown/inactive CLOID: {}",
                    cloid_val
                );
            }
        } else {
            debug!(
                "[SPOT_GRID] Fill received without CLOID at price {}",
                fill.price
            );
        }

        Ok(())
    }

    fn on_order_failed(&mut self, cloid: Cloid, _ctx: &mut StrategyContext) -> Result<()> {
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
        ctx.update_spot_balance("HYPE".to_string(), 100.0, 100.0); // Sufficient base
        ctx.update_spot_balance("USDC".to_string(), 1000.0, 1000.0); // Sufficient quote

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
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: fill_size,
                    price: fill_price,
                    fee,
                    cloid: Some(acq_cloid),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        // Verify that zones waiting to sell now have entry_price = fill_price
        for zone in &strategy.zones {
            if zone.pending_side.is_sell() {
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
        ctx.update_spot_balance("HYPE".to_string(), 100.0, 100.0); // Sufficient base
        ctx.update_spot_balance("USDC".to_string(), 1000.0, 1000.0); // Sufficient quote

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

        // Let's pick Zone 1 (95-100). It should be waiting to buy.
        // Find the active order for Zone 1.
        let zone_idx = 1;
        let zone = &strategy.zones[zone_idx];
        assert_eq!(zone.pending_side, OrderSide::Buy);
        let order_id = zone.order_id.expect("Zone 1 should have an order");

        // 2. Fill Buy Order
        // Price: 95.0. Size: zone.size. Fee: 0.1.
        let fill_price = 95.0;
        let fill_size = zone.size;
        let buy_fee = 0.1;

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: fill_size,
                    price: fill_price,
                    fee: buy_fee,
                    cloid: Some(order_id),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        // Verify Buy Metrics
        assert_eq!(strategy.total_fees, buy_fee);
        let zone = &strategy.zones[zone_idx];
        assert_eq!(zone.pending_side, OrderSide::Sell);
        assert_eq!(zone.entry_price, fill_price);

        // Get new Sell Order ID
        let sell_order_id = zone.order_id.expect("Zone 1 should have sell order");

        // 3. Fill Sell Order
        // Price: 100.0. Fee: 0.1.
        let sell_price = 100.0;

        let sell_fee = 0.1;

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Sell,
                    size: fill_size,
                    price: sell_price,
                    fee: sell_fee,
                    cloid: Some(sell_order_id),
                    reduce_only: None,
                    raw_dir: None,
                },
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
        assert_eq!(zone.pending_side, OrderSide::Buy); // Reset to buy
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
        ctx.update_spot_balance("HYPE".to_string(), 0.0, 0.0); // Start with 0 for tracking test consistency
        ctx.update_spot_balance("USDC".to_string(), 2000.0, 2000.0); // Sufficient quote

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
        zone.pending_side = OrderSide::Buy;
        let order_id = Cloid::new();
        zone.order_id = Some(order_id);
        strategy.active_orders.insert(order_id, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: 10.0,
                    price: 100.0,
                    fee: 0.1,
                    cloid: Some(order_id),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        assert_eq!(strategy.inventory, 10.0);
        assert_eq!(strategy.avg_entry_price, 100.0);

        // 3. Buy 10 @ 110 (Artificial fill to test logic)
        // Reset zone to Buy for test
        let zone = &mut strategy.zones[zone_idx];
        zone.pending_side = OrderSide::Buy;
        let order_id_2 = Cloid::new();
        zone.order_id = Some(order_id_2);
        strategy.active_orders.insert(order_id_2, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: 10.0,
                    price: 110.0,
                    fee: 0.1,
                    cloid: Some(order_id_2),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        assert_eq!(strategy.inventory, 20.0);
        assert_eq!(strategy.avg_entry_price, 105.0); // (10*100 + 10*110) / 20 = 105

        // 4. Sell 5 @ 120
        let zone = &mut strategy.zones[zone_idx];
        zone.pending_side = OrderSide::Sell; // Force to sell
        let order_id_3 = Cloid::new();
        zone.order_id = Some(order_id_3);
        strategy.active_orders.insert(order_id_3, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Sell,
                    size: 5.0,
                    price: 120.0,
                    fee: 0.1,
                    cloid: Some(order_id_3),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        assert_eq!(strategy.inventory, 15.0);
        assert_eq!(strategy.avg_entry_price, 105.0); // Stays same on sell
    }

    #[test]
    fn test_spot_grid_acquisition_sell() {
        // Scenario: High Base, Low Quote.
        // Expect: Sell excess base to cover quote requirements.

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

        // Initial state: 100 HYPE (val: $10k), but 0 USDC.
        // Grid requires USDC for buy levels.
        ctx.update_spot_balance("HYPE".to_string(), 100.0, 100.0);
        ctx.update_spot_balance("USDC".to_string(), 0.0, 0.0);

        if let Some(info) = ctx.market_info_mut("HYPE/USDC") {
            info.last_price = 100.0;
        }

        // Tick 1: Init -> Should detect quote deficit and place SELL order
        strategy.on_tick(100.0, &mut ctx).unwrap();

        match strategy.state {
            StrategyState::AcquiringAssets { .. } => (),
            _ => panic!("Expected AcquiringAssets (Sell), got {:?}", strategy.state),
        }

        // Check placed order
        assert_eq!(ctx.order_queue.len(), 1);
        match &ctx.order_queue[0] {
            crate::model::OrderRequest::Limit { side, sz, .. } => {
                assert_eq!(*side, OrderSide::Sell, "Expected a SELL order for rebalancing");
                assert!(*sz > 0.0);
            }
            _ => panic!("Expected Limit order"),
        }

        // Simulate Fill
        let acq_cloid = match strategy.state {
            StrategyState::AcquiringAssets { cloid } => cloid,
            _ => panic!("Lost state"),
        };

        // Before fill, inventory was 100
        assert_eq!(strategy.inventory, 100.0);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Sell,
                    size: 5.0,
                    price: 105.0,
                    fee: 0.1,
                    cloid: Some(acq_cloid),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        // After sell fill, inventory should be 95
        assert_eq!(strategy.inventory, 95.0);
        assert_eq!(strategy.state, StrategyState::Running);
    }
}
