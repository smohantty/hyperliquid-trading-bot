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

use crate::constants::{ACQUISITION_SPREAD, FEE_BUFFER, INVESTMENT_BUFFER_SPOT};
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
    order_side: OrderSide,
    entry_price: f64,
    cloid: Option<Cloid>,
    roundtrip_count: u32,
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
    initial_entry_price: Option<f64>,
    trigger_reference_price: Option<f64>,
    start_time: Instant,

    // Performance Metrics
    matched_profit: f64,
    total_fees: f64,
    initial_equity: f64,

    // Position Tracking
    inventory_base: f64,
    inventory_quote: f64,

    // Acquisition State Tracking
    acquisition_target_size: f64,
    required_base: f64,
    required_quote: f64,
    initial_avail_base: f64,
    initial_avail_quote: f64,

    // Current Price
    current_price: f64,

    // Grid Configuration (cached)
    grid_count: u32,
    grid_spacing_pct: (f64, f64),
}

impl SpotGridStrategy {
    // =========================================================================
    // INITIALIZATION
    // =========================================================================

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

        // Calculate grid_count and grid_spacing_pct
        let (grid_count, grid_spacing_pct) = if let Some(spread_bips) = config.spread_bips {
            let prices = common::calculate_grid_prices_by_spread(
                config.lower_price,
                config.upper_price,
                spread_bips,
            );
            let spacing = spread_bips / 100.0;
            (prices.len() as u32, (spacing, spacing))
        } else {
            let count = config.grid_count.unwrap_or(2);
            let spacing = common::calculate_grid_spacing_pct(
                &config.grid_type,
                config.lower_price,
                config.upper_price,
                count,
            );
            (count, spacing)
        };

        Self {
            config,
            base_asset,
            quote_asset,
            zones: Vec::new(),
            active_orders: HashMap::new(),
            state: StrategyState::Initializing,
            initial_entry_price: None,
            trigger_reference_price: None,
            start_time: Instant::now(),
            matched_profit: 0.0,
            total_fees: 0.0,
            initial_equity: 0.0,
            inventory_base: 0.0,
            inventory_quote: 0.0,
            acquisition_target_size: 0.0,
            required_base: 0.0,
            required_quote: 0.0,
            initial_avail_base: 0.0,
            initial_avail_quote: 0.0,
            current_price: 0.0,
            grid_count,
            grid_spacing_pct,
        }
    }

    // =========================================================================
    // GRID SETUP & INITIALIZATION
    // =========================================================================

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

        // Store required amounts in struct for debugging/state inspection
        self.required_base = total_base_required;
        self.required_quote = total_quote_required;

        info!(
            "[SPOT_GRID] INITIALIZATION: Asset Required: {} ( {} ), {} ( {} )",
            self.base_asset, total_base_required, self.quote_asset, total_quote_required
        );

        // Track initial inventory (critical for PnL tracking)
        let available_base = ctx.get_spot_available(&self.base_asset);
        let available_quote = ctx.get_spot_available(&self.quote_asset);
        self.inventory_base = available_base;
        self.inventory_quote = available_quote;

        // Store initial available amounts for acquisition tracking
        self.initial_avail_base = available_base;
        self.initial_avail_quote = available_quote;

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
                self.config.grid_type,
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
        let adjusted_investment = INVESTMENT_BUFFER_SPOT.markdown(self.config.total_investment);
        let quote_per_zone = adjusted_investment / num_zones as f64;

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
            let zone_buy_price = prices[i];
            let zone_sell_price = prices[i + 1];

            let raw_size = quote_per_zone / zone_buy_price;
            let size = market_info.round_size(raw_size);

            let order_side = if zone_buy_price > initial_price {
                OrderSide::Sell
            } else {
                OrderSide::Buy
            };

            if order_side.is_sell() {
                total_base_required += size;
            } else {
                total_quote_required += size * zone_buy_price;
            }

            self.zones.push(GridZone {
                index: i,
                buy_price: zone_buy_price,
                sell_price: zone_sell_price,
                size,
                order_side,
                entry_price: if order_side.is_sell() {
                    zone_buy_price
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

    fn calculate_acquisition_price(
        &self,
        side: OrderSide,
        current_price: f64,
        market_info: &MarketInfo,
    ) -> f64 {
        // If trigger_price is set, use it
        if let Some(trigger) = self.config.trigger_price {
            return market_info.round_price(trigger);
        }

        if side.is_buy() {
            // Find nearest level LOWER than market to buy at (Limit Buy below market)
            let candidates: Vec<f64> = self
                .zones
                .iter()
                .filter(|z| z.buy_price < current_price)
                .map(|z| z.buy_price)
                .collect();

            if !candidates.is_empty() {
                return market_info.round_price(candidates.into_iter().fold(0.0, f64::max));
            } else if !self.zones.is_empty() {
                // Fallback: Price is below grid. Return markdown of current price for BUY.
                return market_info.round_price(ACQUISITION_SPREAD.markdown(current_price));
            }
        } else {
            // SELL: Find nearest level ABOVE market to sell at (Limit Sell above market)
            let candidates: Vec<f64> = self
                .zones
                .iter()
                .filter(|z| z.sell_price > current_price)
                .map(|z| z.sell_price)
                .collect();

            if !candidates.is_empty() {
                return market_info
                    .round_price(candidates.into_iter().fold(f64::INFINITY, f64::min));
            } else if !self.zones.is_empty() {
                // Fallback: Price is above grid. Return markup of current price for SELL.
                return market_info.round_price(ACQUISITION_SPREAD.markup(current_price));
            }
        }

        current_price
    }

    fn transition_to_running(&mut self, ctx: &mut StrategyContext, price: f64) {
        self.initial_entry_price = Some(price);
        self.initial_equity = (self.inventory_base * price) + self.inventory_quote;
        self.state = StrategyState::Running;
        self.refresh_orders(ctx);
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

        if base_deficit > 0.0 {
            // Add fee buffer
            let base_deficit = FEE_BUFFER.markup(base_deficit);

            let acquisition_price = self.calculate_acquisition_price(
                OrderSide::Buy,
                market_info.last_price,
                market_info,
            );

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

                let cloid = ctx.place_order(OrderRequest::Limit {
                    symbol: self.config.symbol.clone(),
                    side: OrderSide::Buy,
                    price: acquisition_price,
                    sz: rounded_deficit,
                    reduce_only: false,
                    cloid: None,
                });
                self.state = StrategyState::AcquiringAssets { cloid };
                return Ok(());
            }
        } else if quote_deficit > 0.0 {
            let acquisition_price = self.calculate_acquisition_price(
                OrderSide::Sell,
                market_info.last_price,
                market_info,
            );

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

                let cloid = ctx.place_order(OrderRequest::Limit {
                    symbol: self.config.symbol.clone(),
                    side: OrderSide::Sell,
                    price: acquisition_price,
                    sz: rounded_sell_sz,
                    reduce_only: false,
                    cloid: None,
                });
                self.state = StrategyState::AcquiringAssets { cloid };
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
            self.transition_to_running(ctx, market_info.last_price);
        }

        Ok(())
    }

    // =========================================================================
    // ORDER MANAGEMENT
    // =========================================================================

    fn refresh_orders(&mut self, ctx: &mut StrategyContext) {
        let zones_needing_orders: Vec<usize> = (0..self.zones.len())
            .filter(|&i| self.zones[i].cloid.is_none())
            .collect();

        for zone_idx in zones_needing_orders {
            self.place_zone_order(zone_idx, ctx);
        }
    }

    fn place_zone_order(&mut self, zone_idx: usize, ctx: &mut StrategyContext) {
        let zone = &self.zones[zone_idx];

        if zone.cloid.is_some() {
            return;
        }

        if zone.retry_count >= crate::constants::MAX_ORDER_RETRIES {
            return;
        }

        let side = zone.order_side;
        let price = if side.is_buy() {
            zone.buy_price
        } else {
            zone.sell_price
        };

        let market_info = match ctx.market_info(&self.config.symbol) {
            Some(i) => i,
            None => {
                error!("[SPOT_GRID] No market info for {}", self.config.symbol);
                return;
            }
        };

        let raw_size = if side.is_sell() {
            FEE_BUFFER.markdown(zone.size)
        } else {
            zone.size
        };
        let size = market_info.round_size(raw_size);

        if size <= 0.0 {
            warn!("Calculated size is 0 for zone {}, skipping order", zone_idx);
            return;
        }

        let cloid = ctx.place_order(OrderRequest::Limit {
            symbol: self.config.symbol.clone(),
            side,
            price,
            sz: size,
            reduce_only: false,
            cloid: None,
        });

        self.zones[zone_idx].cloid = Some(cloid);
        self.active_orders.insert(cloid, zone_idx);

        info!(
            "[ORDER_REQUEST] [SPOT_GRID] GRID_ZONE_{} cloid: {} LIMIT {} {} {} @ {}",
            zone_idx, cloid, side, size, self.base_asset, price
        );
    }

    // =========================================================================
    // INTERNAL HELPERS
    // =========================================================================

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

        if fill.side.is_buy() {
            let new_real_base = self.initial_avail_base + fill.size;
            let new_real_quote = self.initial_avail_quote - (fill.size * fill.price);
            self.inventory_base = new_real_base.min(self.required_base).max(0.0);
            self.inventory_quote = new_real_quote.min(self.required_quote).max(0.0);
        } else {
            let new_real_base = self.initial_avail_base - fill.size;
            let new_real_quote = self.initial_avail_quote + (fill.size * fill.price);
            self.inventory_base = new_real_base.min(self.required_base).max(0.0);
            self.inventory_quote = new_real_quote.min(self.required_quote).max(0.0);
        }

        self.transition_to_running(ctx, fill.price);
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

        self.total_fees += fill.fee;
        self.zones[zone_idx].retry_count = 0;

        self.inventory_base += fill.size;
        self.inventory_quote -= fill.price * fill.size;

        self.zones[zone_idx].order_side = OrderSide::Sell;
        self.zones[zone_idx].entry_price = fill.price;

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

        self.zones[zone_idx].roundtrip_count += 1;
        self.zones[zone_idx].retry_count = 0;

        self.matched_profit += pnl;
        self.total_fees += fill.fee;

        self.inventory_base = (self.inventory_base - fill.size).max(0.0);
        self.inventory_quote += fill.price * fill.size;

        self.zones[zone_idx].order_side = OrderSide::Buy;
        self.zones[zone_idx].entry_price = 0.0;

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

        let next_cloid = ctx.place_order(OrderRequest::Limit {
            symbol: self.config.symbol.clone(),
            side,
            price,
            sz: zone.size,
            reduce_only: false,
            cloid: None,
        });

        self.active_orders.insert(next_cloid, zone_idx);
        self.zones[zone_idx].cloid = Some(next_cloid);

        info!(
            "[ORDER_REQUEST] [SPOT_GRID] COUNTER_ORDER: cloid: {} LIMIT {} {} {} @ {}",
            next_cloid, side, self.zones[zone_idx].size, self.config.symbol, price
        );

        Ok(())
    }
    fn validate_fill(&self, zone_idx: usize, fill: &OrderFill) {
        let expected_side = self.zones[zone_idx].order_side;

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

// =============================================================================
// STRATEGY LIFECYCLE (Trait Implementation)
// =============================================================================

impl Strategy for SpotGridStrategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()> {
        self.current_price = price;

        match self.state {
            StrategyState::Initializing => {
                self.initialize_zones(ctx)?;
            }
            StrategyState::AcquiringAssets { .. } => {
                // Handled in on_order_filled or wait for transition
            }
            StrategyState::WaitingForTrigger => {
                if let (Some(trigger), Some(start)) =
                    (self.config.trigger_price, self.trigger_reference_price)
                {
                    if common::check_trigger(price, trigger, start) {
                        info!("[SPOT_GRID] [Triggered] at {}", price);
                        self.transition_to_running(ctx, price);
                    }
                }
            }
            StrategyState::Running => {
                self.refresh_orders(ctx);
            }
        }

        Ok(())
    }

    fn on_order_filled(&mut self, fill: &OrderFill, ctx: &mut StrategyContext) -> Result<()> {
        if self.state == StrategyState::Initializing {
            return Ok(());
        }

        if let Some(cloid_val) = fill.cloid {
            if let StrategyState::AcquiringAssets { cloid: acq_cloid } = self.state {
                if cloid_val == acq_cloid {
                    return self.handle_acquisition_fill(fill, ctx);
                }
            }

            if let Some(zone_idx) = self.active_orders.remove(&cloid_val) {
                {
                    let zone = &self.zones[zone_idx];
                    if zone.cloid != Some(cloid_val) {
                        warn!(
                            "[SPOT_GRID] Zone {} cloid mismatch! Expected {:?}, got {}",
                            zone_idx, zone.cloid, cloid_val
                        );
                    }
                }

                self.validate_fill(zone_idx, fill);
                let expected_side = self.zones[zone_idx].order_side;

                self.zones[zone_idx].cloid = None;

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
        if self.state == StrategyState::Initializing {
            return Ok(());
        }

        if let Some(zone_idx) = self.active_orders.remove(&cloid) {
            if let Some(zone) = self.zones.get_mut(zone_idx) {
                if zone.cloid == Some(cloid) {
                    zone.cloid = None;
                    zone.retry_count += 1;

                    log::warn!(
                        "[ORDER_FAILED] [SPOT_GRID] GRID_ZONE_{} cloid: {} Retry count: {}/{}",
                        zone_idx,
                        cloid,
                        zone.retry_count,
                        crate::constants::MAX_ORDER_RETRIES
                    );
                }
            }
        }
        Ok(())
    }

    fn get_summary(&self, _ctx: &StrategyContext) -> StrategySummary {
        if self.state == StrategyState::Initializing {
            panic!("Strategy not initialized");
        }

        use crate::broadcast::types::SpotGridSummary;

        let total_profit = if self.state == StrategyState::Running {
            let current_equity = (self.inventory_base * self.current_price) + self.inventory_quote;
            current_equity - self.initial_equity - self.total_fees
        } else {
            0.0
        };

        let total_roundtrips: u32 = self.zones.iter().map(|z| z.roundtrip_count).sum();

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
            grid_count: self.grid_count,
            range_low: self.config.lower_price,
            range_high: self.config.upper_price,
            grid_spacing_pct: self.grid_spacing_pct,
            roundtrips: total_roundtrips,
            base_balance: self.inventory_base,
            quote_balance: self.inventory_quote,
        })
    }

    fn get_grid_state(&self, _ctx: &StrategyContext) -> GridState {
        if self.state == StrategyState::Initializing {
            panic!("Strategy not initialized");
        }

        use crate::broadcast::types::ZoneInfo;

        let zones = self
            .zones
            .iter()
            .map(|z| ZoneInfo {
                index: z.index,
                buy_price: z.buy_price,
                sell_price: z.sell_price,
                size: z.size,
                order_side: z.order_side.to_string(),
                has_order: z.cloid.is_some(),
                is_reduce_only: false,
                entry_price: z.entry_price,
                roundtrip_count: z.roundtrip_count,
            })
            .collect();

        GridState {
            symbol: self.config.symbol.clone(),
            strategy_type: "spot_grid".to_string(),
            grid_bias: None,
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

        // Verify that zones waiting to sell have entry_price = zone's buy_price (not fill_price)
        // This is the new behavior aligned with Python: entry_price represents the buy level for profit calculation
        for zone in &strategy.zones {
            if zone.order_side.is_sell() {
                assert_eq!(
                    zone.entry_price, zone.buy_price,
                    "Zone {} entry price should be buy_price",
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

        // After sell fill, inventory_base is capped to required_base (which is the base needed for sell zones)
        // With grid [90-95], [95-100], [100-105], [105-110] at price 100:
        // - Zones [100-105] and [105-110] have buy_price > 100, so they are SELL zones needing base
        // inventory_base = min(initial_avail_base - fill.size, required_base)
        assert_eq!(strategy.inventory_base, strategy.required_base);
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
