use super::common;

use crate::broadcast::types::{GridState, StrategySummary};
use crate::config::strategy::SpotGridConfig;

use crate::engine::context::{MarketInfo, StrategyContext, MIN_NOTIONAL_VALUE};
use crate::model::{Cloid, OrderFill, OrderRequest, OrderSide};
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::time::Instant;

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
    buy_price: f64,
    sell_price: f64,
    size: f64,
    /// The side of the pending order for this zone (Buy at buy_price, Sell at sell_price)
    order_side: OrderSide,
    entry_price: f64,
    cloid: Option<Cloid>,

    // Performance Metrics
    roundtrip_count: u32,
    /// Track order failures for retry logic
    retry_count: u32,
}

#[allow(dead_code)]
pub struct SpotGridStrategy {
    pub config: SpotGridConfig,
    base_asset: String,
    quote_asset: String,

    // Internal State
    zones: Vec<GridZone>,
    active_orders: HashMap<Cloid, usize>,
    state: StrategyState,
    trade_count: u32,
    initial_entry_price: Option<f64>,
    trigger_reference_price: Option<f64>,
    start_time: Instant,

    // Performance Metrics (aligned with Python)
    matched_profit: f64, // Profit from completed roundtrips
    total_fees: f64,
    initial_equity: f64, // Starting equity for total_profit calculation

    // Position Tracking
    inventory_base: f64,
    inventory_quote: f64,
}

impl SpotGridStrategy {
    pub fn new(config: SpotGridConfig) -> Self {
        // Parse symbol (e.g., "HYPE/USDC")
        let parts: Vec<&str> = config.symbol.split('/').collect();
        let (base_asset, quote_asset) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            warn!(
                "Invalid symbol format: {}. Assuming base/USDC.",
                config.symbol
            );
            (config.symbol.clone(), "USDC".to_string())
        };

        Self {
            config,
            base_asset,
            quote_asset,
            zones: Vec::new(),
            active_orders: HashMap::new(),
            trade_count: 0,
            state: StrategyState::Initializing,
            initial_entry_price: None,
            trigger_reference_price: None,
            start_time: Instant::now(),
            matched_profit: 0.0,
            total_fees: 0.0,
            initial_equity: 0.0,
            inventory_base: 0.0,
            inventory_quote: 0.0,
        }
    }

    fn initialize_zones(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        self.config.validate().map_err(|e| anyhow!(e))?;

        let market_info = match ctx.market_info(&self.config.symbol) {
            Some(info) => info.clone(),
            None => {
                return Err(anyhow!("No market info for {}", self.config.symbol));
            }
        };

        // 1. Generate Zones
        let (total_base_required, total_quote_required) = self.calculate_grid_plan(&market_info)?;

        info!(
            "[SPOT_GRID] INITIALIZATION: Asset Required: {} ( {} ), {} ( {} )",
            self.base_asset, total_base_required, self.quote_asset, total_quote_required
        );

        // Track initial inventory (critical for PnL tracking)
        let available_base = ctx.get_spot_available(&self.base_asset);
        let available_quote = ctx.get_spot_available(&self.quote_asset);
        self.inventory_base = available_base;
        self.inventory_quote = available_quote;

        // Calculate and store initial equity
        let initial_price = self.config.trigger_price.unwrap_or(market_info.last_price);
        self.initial_equity = (self.inventory_base * initial_price) + self.inventory_quote;

        if self.inventory_base > 0.0 {
            info!(
                "[SPOT_GRID] Initial inventory detected: {} {}, {} {}",
                self.inventory_base, self.base_asset, self.inventory_quote, self.quote_asset
            );
        }

        // Upfront Total Investment Validation
        // Calculate approx market value of our total holdings for this strategy.
        // We use the initial_price (which considers trigger price) as that is the price
        // at which the asset requirements are calculated.
        let initial_price = self.config.trigger_price.unwrap_or(market_info.last_price);
        let total_wallet_value = (available_base * initial_price) + available_quote;

        if total_wallet_value < self.config.total_investment {
            let msg = format!(
                "Insufficient Total Portfolio Value! Required: {:.2} {}, Have approx: {:.2} {} ({} {} + {} {}). Bailing out.",
                self.config.total_investment, self.quote_asset, total_wallet_value, self.quote_asset,
                available_base, self.base_asset, available_quote, self.quote_asset
            );
            error!("[SPOT_GRID] {}", msg);
            return Err(anyhow!(msg));
        }

        // 2. Check Assets & Rebalance if necessary
        self.check_initial_acquisition(ctx, &market_info, total_base_required, total_quote_required)
    }

    /// Calculate grid plan - generates zones based on grid_count OR spread_bips
    fn calculate_grid_plan(&mut self, market_info: &MarketInfo) -> Result<(f64, f64)> {
        // Generate price levels based on config
        let prices: Vec<f64> = if let Some(spread_bips) = self.config.spread_bips {
            // Use spread_bips to calculate levels
            common::calculate_grid_prices_by_spread(
                self.config.lower_price,
                self.config.upper_price,
                spread_bips,
            )
            .into_iter()
            .map(|p| market_info.round_price(p))
            .collect()
        } else if let Some(count) = self.config.grid_count {
            // Use grid_count
            common::calculate_grid_prices(
                self.config.grid_type.clone(),
                self.config.lower_price,
                self.config.upper_price,
                count,
            )
            .into_iter()
            .map(|p| market_info.round_price(p))
            .collect()
        } else {
            return Err(anyhow!(
                "Either grid_count or spread_bips must be specified"
            ));
        };

        if prices.len() < 2 {
            return Err(anyhow!("Not enough price levels generated"));
        }

        let num_zones = prices.len() - 1;
        let quote_per_zone = self.config.total_investment / num_zones as f64;

        if quote_per_zone < MIN_NOTIONAL_VALUE {
            let msg = format!(
                "Quote per zone ({:.2}) is less than minimum order value ({}). Increase total_investment or decrease grid_count.",
                quote_per_zone, MIN_NOTIONAL_VALUE
            );
            error!("[SPOT_GRID] {}", msg);
            return Err(anyhow!(msg));
        }

        // Use trigger_price if available, otherwise last_price
        let initial_price = self.config.trigger_price.unwrap_or(market_info.last_price);

        self.zones.clear();
        let mut total_base_required = 0.0;
        let mut total_quote_required = 0.0;

        for i in 0..num_zones {
            let lower = prices[i];
            let upper = prices[i + 1];

            // Calculate size based on quote investment per zone
            let raw_size = quote_per_zone / lower;
            let size = market_info.round_size(raw_size);

            // Zone ABOVE price line (buy_price > price): We acquired base at initial_price -> Sell at sell_price
            // Zone AT or BELOW price line: We have quote, waiting to buy at buy_price
            let order_side = if lower > initial_price {
                OrderSide::Sell // Zone above price line -> sell at upper, then ping-pong
            } else {
                OrderSide::Buy // Zone at/below price line -> buy at lower, then ping-pong
            };

            if order_side.is_sell() {
                total_base_required += size;
            } else {
                total_quote_required += size * lower;
            }

            self.zones.push(GridZone {
                index: i,
                buy_price: lower,
                sell_price: upper,
                size,
                order_side,
                entry_price: if order_side.is_sell() {
                    initial_price
                } else {
                    0.0
                },
                cloid: None,
                roundtrip_count: 0,
                retry_count: 0,
            });
        }

        info!("[SPOT_GRID] Grid Plan: {} zones generated", num_zones);

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
        let initial_price = self.config.trigger_price.unwrap_or(market_info.last_price);

        if base_deficit > 0.0 {
            // Case 1: Not enough base asset (e.g. BTC) to cover the SELL levels.
            // Need to BUY base asset.
            let mut acquisition_price = initial_price;

            if let Some(trigger) = self.config.trigger_price {
                acquisition_price = market_info.round_price(trigger);
            } else {
                let nearest_level = self
                    .zones
                    .iter()
                    .filter(|z| z.buy_price < market_info.last_price)
                    .map(|z| z.buy_price)
                    .fold(0.0, f64::max);

                if nearest_level > 0.0 {
                    acquisition_price = market_info.round_price(nearest_level);
                } else if !self.zones.is_empty() {
                    acquisition_price = market_info.round_price(self.zones[0].buy_price);
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
                    symbol: self.config.symbol.clone(),
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

            if let Some(trigger) = self.config.trigger_price {
                acquisition_price = market_info.round_price(trigger);
            } else {
                // Find nearest level ABOVE market to sell at
                let nearest_sell_level = self
                    .zones
                    .iter()
                    .filter(|z| z.sell_price > market_info.last_price)
                    .map(|z| z.sell_price)
                    .fold(f64::INFINITY, f64::min);

                if nearest_sell_level.is_finite() {
                    acquisition_price = market_info.round_price(nearest_sell_level);
                } else if !self.zones.is_empty() {
                    acquisition_price =
                        market_info.round_price(self.zones.last().unwrap().sell_price);
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
                    symbol: self.config.symbol.clone(),
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
        if let Some(_trigger) = self.config.trigger_price {
            // Passive Wait Mode
            info!("[SPOT_GRID] Assets sufficient. Entering WaitingForTrigger state.");
            self.trigger_reference_price = Some(market_info.last_price);
            self.state = StrategyState::WaitingForTrigger;
        } else {
            // No Trigger, Assets OK -> Running
            info!("[SPOT_GRID] Assets verified. Starting Grid.");
            self.initial_entry_price = Some(market_info.last_price);
            self.state = StrategyState::Running;
        }

        Ok(())
    }

    fn refresh_orders(&mut self, ctx: &mut StrategyContext) {
        // Collect orders to place to avoid borrowing issues
        let mut orders_to_place: Vec<(usize, OrderSide, f64, f64, Cloid)> = Vec::new();

        for i in 0..self.zones.len() {
            if self.zones[i].cloid.is_none() {
                let zone = &self.zones[i];
                let price = if zone.order_side.is_buy() {
                    zone.buy_price
                } else {
                    zone.sell_price
                };

                let cloid = ctx.generate_cloid();
                orders_to_place.push((i, zone.order_side, price, zone.size, cloid));
            }
        }

        // Execute placement
        for (index, side, price, size, cloid) in orders_to_place {
            let zone = &mut self.zones[index];
            zone.cloid = Some(cloid);
            self.active_orders.insert(cloid, index);

            info!(
                "[ORDER_REQUEST] [SPOT_GRID] GRID_LVL_{}: LIMIT {} {} {} @ {}",
                index, side, size, self.config.symbol, price
            );
            ctx.place_order(OrderRequest::Limit {
                symbol: self.config.symbol.clone(),
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
            if fill.side.is_buy() {
                "Purchased"
            } else {
                "Sold"
            },
            fill.size,
            fill.price,
            fill.fee
        );
        self.total_fees += fill.fee;

        // Update Inventory
        if fill.side.is_buy() {
            self.inventory_base += fill.size;
            self.inventory_quote -= fill.price * fill.size;
        } else {
            self.inventory_base = (self.inventory_base - fill.size).max(0.0);
            self.inventory_quote += fill.price * fill.size;
        }

        // Update entry_price for all zones waiting to sell to the actual fill price
        // (they now have inventory at this cost basis)
        for zone in &mut self.zones {
            if zone.order_side.is_sell() {
                zone.entry_price = fill.price;
            }
        }

        self.initial_entry_price = Some(fill.price);
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
        let next_price = self.zones[zone_idx].sell_price;

        info!(
            "[SPOT_GRID] Zone {} | BUY Filled @ {} | Size: {} | Fee: {:.4} | Next: SELL @ {}",
            zone_idx, fill.price, fill.size, fill.fee, next_price
        );

        // Update Strategy Fees
        self.total_fees += fill.fee;

        // Buy Fill: Increase base inventory, decrease quote inventory
        self.inventory_base += fill.size;
        self.inventory_quote -= fill.price * fill.size;

        // Update zone: now waiting to sell
        self.zones[zone_idx].order_side = OrderSide::Sell;
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
        let next_price = zone.buy_price;

        info!(
            "[SPOT_GRID] Zone {} | SELL Filled @ {} | Size: {} | PnL: {:.4} | Fee: {:.4} | Next: BUY @ {}",
            zone_idx, fill.price, fill.size, pnl, fill.fee, next_price
        );

        // Update Zone Metrics
        self.zones[zone_idx].roundtrip_count += 1;

        // Update Strategy Metrics
        self.matched_profit += pnl;
        self.total_fees += fill.fee;

        // Sell Fill: Decrease Inventory, increase quote
        self.inventory_base = (self.inventory_base - fill.size).max(0.0);
        self.inventory_quote += fill.price * fill.size;

        // Update zone: now waiting to buy
        self.zones[zone_idx].order_side = OrderSide::Buy;
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
            side, zone.size, self.config.symbol, price
        );

        self.active_orders.insert(next_cloid, zone_idx);
        zone.cloid = Some(next_cloid);

        ctx.place_order(OrderRequest::Limit {
            symbol: self.config.symbol.clone(),
            side,
            price,
            sz: zone.size,
            reduce_only: false,
            cloid: Some(next_cloid),
        });

        Ok(())
    }
    fn validate_fill(&self, zone_idx: usize, fill: &OrderFill) {
        let expected_side = self.zones[zone_idx].order_side;

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
            let expected_dir = if expected_side.is_buy() {
                "Buy"
            } else {
                "Sell"
            };
            if raw_dir != expected_dir {
                error!(
                    "[SPOT_GRID] ASSERTION FAILED: Zone {} expected raw_dir '{}' but got '{}'",
                    zone_idx, expected_dir, raw_dir
                );
            }
        }
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
                if let Some(trigger) = self.config.trigger_price {
                    // Directional Trigger Logic
                    // Requires start_price to be set during initialization

                    let start = self.trigger_reference_price.expect(
                        "Trigger reference price must be set when in WaitingForTrigger state",
                    );

                    if common::check_trigger(price, trigger, start) {
                        info!(
                            "[SPOT_GRID] Price {} crossed trigger {}. Starting.",
                            price, trigger
                        );
                        self.initial_entry_price = Some(price);
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
                    if zone.cloid != Some(cloid_val) {
                        warn!(
                            "[SPOT_GRID] Zone {} cloid mismatch! Expected {:?}, got {}",
                            zone_idx, zone.cloid, cloid_val
                        );
                    }
                }

                // Validate Fill Expectations
                self.validate_fill(zone_idx, fill);
                let expected_side = self.zones[zone_idx].order_side;

                // Update Zone State
                self.zones[zone_idx].cloid = None;
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
        if let Some(zone_idx) = self.active_orders.remove(&cloid) {
            if let Some(zone) = self.zones.get_mut(zone_idx) {
                if zone.cloid == Some(cloid) {
                    log::info!(
                        "[SPOT_GRID] Clearing failed order state for Zone {}",
                        zone_idx
                    );
                    zone.cloid = None;
                }
            }
        }
        Ok(())
    }

    fn get_summary(&self, ctx: &StrategyContext) -> StrategySummary {
        use crate::broadcast::types::SpotGridSummary;

        let current_price = ctx
            .market_info(&self.config.symbol)
            .map(|m| m.last_price)
            .unwrap_or(0.0);

        // Calculate total_profit: current_equity - initial_equity - fees
        let current_equity = (self.inventory_base * current_price) + self.inventory_quote;
        let total_profit = current_equity - self.initial_equity - self.total_fees;

        // Calculate total roundtrips from zones
        let total_roundtrips: u32 = self.zones.iter().map(|z| z.roundtrip_count).sum();

        // Calculate grid spacing percentage
        // Use zones.len() + 1 as effective grid_count (zones are between grid levels)
        let grid_count = (self.zones.len() + 1) as u32;
        let grid_spacing_pct = common::calculate_grid_spacing_pct(
            &self.config.grid_type,
            self.config.lower_price,
            self.config.upper_price,
            grid_count,
        );

        // Calculate uptime
        let uptime = common::format_uptime(self.start_time.elapsed());

        StrategySummary::SpotGrid(SpotGridSummary {
            symbol: self.config.symbol.clone(),
            state: format!("{:?}", self.state),
            uptime,
            position_size: self.inventory_base,
            matched_profit: self.matched_profit,
            total_profit,
            total_fees: self.total_fees,
            initial_entry_price: self.initial_entry_price,
            grid_count: self.zones.len() as u32,
            range_low: self.config.lower_price,
            range_high: self.config.upper_price,
            grid_spacing_pct,
            roundtrips: total_roundtrips,
            base_balance: ctx.get_spot_total(&self.base_asset),
            quote_balance: ctx.get_spot_total(&self.quote_asset),
        })
    }

    fn get_grid_state(&self, _ctx: &StrategyContext) -> GridState {
        use crate::broadcast::types::ZoneInfo;

        let zones = self
            .zones
            .iter()
            .map(|z| {
                // Spot grid: Buy = opening, Sell = closing
                ZoneInfo {
                    index: z.index,
                    buy_price: z.buy_price,
                    sell_price: z.sell_price,
                    size: z.size,
                    order_side: z.order_side.to_string(),
                    has_order: z.cloid.is_some(),
                    is_reduce_only: false, // Spot doesn't have reduce_only
                    entry_price: z.entry_price,
                    roundtrip_count: z.roundtrip_count,
                }
            })
            .collect();

        GridState {
            symbol: self.config.symbol.clone(),
            strategy_type: "spot_grid".to_string(),
            grid_bias: None, // Spot has no bias
            zones,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::strategy::SpotGridConfig;
    use crate::engine::context::{MarketInfo, StrategyContext};
    use crate::strategy::types::GridType;

    fn create_test_setup(
        trigger_price: Option<f64>,
        base_balance: f64,
        quote_balance: f64,
        last_price: f64,
    ) -> (SpotGridStrategy, StrategyContext) {
        let config = SpotGridConfig {
            symbol: "HYPE/USDC".to_string(),
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: Some(5),
            spread_bips: None,
            total_investment: 1000.0,
            trigger_price,
        };

        let strategy = SpotGridStrategy::new(config);
        let mut markets = HashMap::new();
        markets.insert(
            "HYPE/USDC".to_string(),
            MarketInfo::new("HYPE/USDC".to_string(), "HYPE".to_string(), 0, 2, 2),
        );
        let mut ctx = StrategyContext::new(markets);
        ctx.update_spot_balance("HYPE".to_string(), base_balance, base_balance);
        ctx.update_spot_balance("USDC".to_string(), quote_balance, quote_balance);

        if let Some(info) = ctx.market_info_mut("HYPE/USDC") {
            info.last_price = last_price;
        }

        (strategy, ctx)
    }

    #[test]
    fn test_spot_grid_passive_trigger() {
        // Scenario: Assets Sufficient. Wait for trigger.
        // Start Price: 100. Trigger: 105. Expect: Wait until > 105.
        let (mut strategy, mut ctx) = create_test_setup(Some(105.0), 100.0, 1000.0, 100.0);

        // Tick 1: Initialization. Should transition to Initializing -> WaitingForTrigger.
        // Strategy starts in Initializing.
        strategy.on_tick(100.0, &mut ctx).unwrap();

        // initialize_zones runs. Finds Assets OK. Trigger Set. -> WaitingForTrigger.
        match strategy.state {
            StrategyState::WaitingForTrigger => (),
            _ => panic!("Expected WaitingForTrigger, got {:?}", strategy.state),
        }
        assert_eq!(strategy.trigger_reference_price, Some(100.0));

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
        //
        // Grid zones with grid_count=5: [90-95], [95-100], [100-105], [105-110]
        // trigger_price=104 means initial_price=104
        // Zone [105-110] has buy_price=105 > 104 → Sell (needs base acquisition)

        let (mut strategy, mut ctx) = create_test_setup(Some(104.0), 0.0, 2000.0, 100.0);

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
                assert_eq!(*price, 104.0); // Should be trigger price
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
            if zone.order_side.is_sell() {
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

        let (mut strategy, mut ctx) = create_test_setup(None, 100.0, 1000.0, 100.0);

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
        assert_eq!(zone.order_side, OrderSide::Buy);
        let cloid = zone.cloid.expect("Zone 1 should have an order");

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
                    cloid: Some(cloid),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        // Verify Buy Metrics
        assert_eq!(strategy.total_fees, buy_fee);
        let zone = &strategy.zones[zone_idx];
        assert_eq!(zone.order_side, OrderSide::Sell);
        assert_eq!(zone.entry_price, fill_price);

        // Get new Sell Order ID
        let sell_cloid = zone.cloid.expect("Zone 1 should have sell order");

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
                    cloid: Some(sell_cloid),
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

        assert_eq!(strategy.matched_profit, expected_pnl);
        assert_eq!(strategy.total_fees, expected_total_fees);

        let zone = &strategy.zones[zone_idx];
        assert_eq!(zone.order_side, OrderSide::Buy); // Reset to buy
        assert_eq!(zone.roundtrip_count, 1);
    }

    #[test]
    fn test_spot_grid_position_tracking() {
        // Scenario: verify Inventory and Avg Entry Price tracking
        // 1. Start with 0 Inventory
        // 2. Buy 10 @ 100 -> Inventory 10, Avg 100
        // 3. Buy 10 @ 110 -> Inventory 20, Avg 105
        // 4. Sell 5 @ 120 -> Inventory 15, Avg 105

        // Start with 0 for tracking test consistency
        let (mut strategy, mut ctx) = create_test_setup(None, 0.0, 2000.0, 100.0);

        // 1. Before Init - inventory is 0
        assert_eq!(strategy.inventory_base, 0.0);
        assert_eq!(strategy.inventory_quote, 0.0); // Before on_tick, inventory_quote is not set

        strategy.on_tick(100.0, &mut ctx).unwrap();

        // Check if zones initialized
        if strategy.zones.is_empty() {
            strategy.on_tick(100.0, &mut ctx).unwrap();
        }
        assert!(!strategy.zones.is_empty(), "Zones should be initialized");

        // After initialization, inventory_quote should be set
        assert_eq!(strategy.inventory_quote, 2000.0);

        // 2. Buy 10 @ 100
        let zone_idx = 0;
        let zone = &mut strategy.zones[zone_idx];
        zone.order_side = OrderSide::Buy;
        let cloid = Cloid::new();
        zone.cloid = Some(cloid);
        strategy.active_orders.insert(cloid, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: 10.0,
                    price: 100.0,
                    fee: 0.1,
                    cloid: Some(cloid),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        // Verify inventory was updated
        assert_eq!(strategy.inventory_base, 10.0);

        // 3. Buy 10 @ 110 (Artificial fill to test logic)
        // Reset zone to Buy for test
        let zone = &mut strategy.zones[zone_idx];
        zone.order_side = OrderSide::Buy;
        let cloid_2 = Cloid::new();
        zone.cloid = Some(cloid_2);
        strategy.active_orders.insert(cloid_2, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: 10.0,
                    price: 110.0,
                    fee: 0.1,
                    cloid: Some(cloid_2),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        assert_eq!(strategy.inventory_base, 20.0);

        // 4. Sell 5 @ 120
        let zone = &mut strategy.zones[zone_idx];
        zone.order_side = OrderSide::Sell; // Force to sell
        let cloid_3 = Cloid::new();
        zone.cloid = Some(cloid_3);
        strategy.active_orders.insert(cloid_3, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Sell,
                    size: 5.0,
                    price: 120.0,
                    fee: 0.1,
                    cloid: Some(cloid_3),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        assert_eq!(strategy.inventory_base, 15.0);
    }

    #[test]
    fn test_spot_grid_acquisition_sell() {
        // Scenario: High Base, Low Quote.
        // Expect: Sell excess base to cover quote requirements.

        // Initial state: 100 HYPE (val: $10k), but 0 USDC.
        let (mut strategy, mut ctx) = create_test_setup(None, 100.0, 0.0, 100.0);

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
                assert_eq!(
                    *side,
                    OrderSide::Sell,
                    "Expected a SELL order for rebalancing"
                );
                assert!(*sz > 0.0);
            }
            _ => panic!("Expected Limit order"),
        }

        // Simulate Fill
        let acq_cloid = match strategy.state {
            StrategyState::AcquiringAssets { cloid } => cloid,
            _ => panic!("Lost state"),
        };

        // Before fill, inventory_base was 100
        assert_eq!(strategy.inventory_base, 100.0);

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

        // After sell fill, inventory_base should be 95
        assert_eq!(strategy.inventory_base, 95.0);
        assert_eq!(strategy.state, StrategyState::Running);
    }

    #[test]
    fn test_spot_grid_order_failure_recovery() {
        // Scenario: Order Fails -> Zone State Cleared -> Retry on next Tick
        let (mut strategy, mut ctx) = create_test_setup(None, 100.0, 1000.0, 100.0);

        // 1. Initialize & Start
        strategy.on_tick(100.0, &mut ctx).unwrap();
        strategy.on_tick(100.0, &mut ctx).unwrap(); // Refresh orders

        // Get a zone with an active order (Zone 0 is below 100, pending Buy)
        // Zone 0: 90-95. Wait Buy.
        // Zone 1: 95-100. Wait Buy.
        // Zone 2: 100-105. Wait Sell (Inventory).
        // Let's pick Zone 0.
        let zone_idx = 0;
        let zone = &strategy.zones[zone_idx];
        let original_cloid = zone.cloid.expect("Zone 0 should have order");
        assert!(strategy.active_orders.contains_key(&original_cloid));

        // 2. Fail the order
        strategy.on_order_failed(original_cloid, &mut ctx).unwrap();

        // 3. Verify State Cleared
        let zone = &strategy.zones[zone_idx];
        assert_eq!(zone.cloid, None, "Zone cloid should be cleared");
        assert!(
            !strategy.active_orders.contains_key(&original_cloid),
            "Active order should be removed"
        );

        // 4. Tick -> Retry
        strategy.on_tick(100.0, &mut ctx).unwrap();

        // 5. Verify New Order
        let zone = &strategy.zones[zone_idx];
        let new_cloid = zone.cloid.expect("Zone 0 should have new order");
        assert_ne!(new_cloid, original_cloid, "Should be a new cloid");
        assert!(strategy.active_orders.contains_key(&new_cloid));
    }
    #[test]
    fn test_spot_grid_avg_price_reset() {
        // Scenario: Buy -> Sell All -> Buy again. Avg Price should reset.
        let (mut strategy, mut ctx) = create_test_setup(None, 0.0, 2000.0, 100.0);

        // Ensure zones are initialized
        strategy.on_tick(100.0, &mut ctx).unwrap();

        // 1. Buy 10 @ 100
        // Pick a zone that would be buying (below 100)
        let zone_idx = 0; // Likely 90-95
        let buy_cloid = Cloid::new();
        strategy.zones[zone_idx].cloid = Some(buy_cloid);
        strategy.zones[zone_idx].order_side = OrderSide::Buy;
        strategy.active_orders.insert(buy_cloid, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: 10.0,
                    price: 100.0,
                    fee: 0.1,
                    cloid: Some(buy_cloid),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(strategy.inventory_base, 10.0);

        // 2. Sell 10 @ 110 (Close position)
        // Set up zone to be selling
        let sell_cloid = Cloid::new();
        strategy.zones[zone_idx].cloid = Some(sell_cloid);
        strategy.zones[zone_idx].order_side = OrderSide::Sell;
        strategy.active_orders.insert(sell_cloid, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Sell,
                    size: 10.0,
                    price: 110.0,
                    fee: 0.1,
                    cloid: Some(sell_cloid),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        assert_eq!(strategy.inventory_base, 0.0);

        // 3. Buy 5 @ 120 (New position)
        let buy_cloid_2 = Cloid::new();
        strategy.zones[zone_idx].cloid = Some(buy_cloid_2);
        strategy.zones[zone_idx].order_side = OrderSide::Buy;
        strategy.active_orders.insert(buy_cloid_2, zone_idx);

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: 5.0,
                    price: 120.0,
                    fee: 0.1,
                    cloid: Some(buy_cloid_2),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(strategy.inventory_base, 5.0);
    }
    #[test]
    fn test_spot_grid_initialization_with_inventory() {
        // Scenario: Start with 10 HYPE (no fetch required).
        // Expect: avg_entry_price initialized to market/trigger price (100.0)
        let (mut strategy, mut ctx) = create_test_setup(None, 10.0, 1000.0, 100.0);

        // Tick to init
        strategy.on_tick(100.0, &mut ctx).unwrap();

        assert_eq!(strategy.inventory_base, 10.0);
    }
}
