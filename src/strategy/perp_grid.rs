use crate::broadcast::types::{GridState, StrategySummary};
use crate::config::strategy::PerpGridConfig;

use crate::engine::context::{StrategyContext, MIN_NOTIONAL_VALUE};
use crate::model::{Cloid, OrderFill, OrderRequest, OrderSide};
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

use super::common;
use super::types::{GridBias, ZoneMode};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum StrategyState {
    Initializing,
    WaitingForTrigger,
    AcquiringAssets { cloid: Cloid, target_size: f64 },
    Running,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GridZone {
    index: usize,
    lower_price: f64,
    upper_price: f64,
    size: f64,
    /// The side of the pending order for this zone
    pending_side: OrderSide,
    /// The operational mode: Long (buy to open) or Short (sell to open)
    mode: ZoneMode,
    entry_price: f64,
    order_id: Option<Cloid>,

    // Performance Metrics
    roundtrip_count: u32,
}

#[allow(dead_code)]
pub struct PerpGridStrategy {
    pub config: PerpGridConfig,

    // Internal State
    zones: Vec<GridZone>,
    active_orders: HashMap<Cloid, usize>, // cloid -> zone_index
    trade_count: u32,
    state: StrategyState,
    start_price: Option<f64>,
    start_time: Instant,

    // Performance Metrics
    realized_pnl: f64,
    total_fees: f64,
    unrealized_pnl: f64,

    // Position Tracking
    position_size: f64,
    avg_entry_price: f64,
}

impl PerpGridStrategy {
    pub fn new(config: PerpGridConfig) -> Self {
        Self {
            config,
            zones: Vec::new(),
            active_orders: HashMap::new(),
            trade_count: 0,
            state: StrategyState::Initializing,
            start_price: None,
            start_time: Instant::now(),
            realized_pnl: 0.0,
            total_fees: 0.0,
            unrealized_pnl: 0.0,
            position_size: 0.0,
            avg_entry_price: 0.0,
        }
    }

    fn initialize_zones(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        self.config.validate().map_err(|e| anyhow!(e))?;

        // 1. Get initial data (scoped)
        let last_price = {
            let market_info = match ctx.market_info(&self.config.symbol) {
                Some(i) => i,
                None => {
                    return Err(anyhow!("No market info for {}", self.config.symbol));
                }
            };
            market_info.last_price
        };

        // Generate Levels
        let prices: Vec<f64> = {
            let market_info = ctx.market_info(&self.config.symbol).unwrap();
            common::calculate_grid_prices(
                self.config.grid_type.clone(),
                self.config.lower_price,
                self.config.upper_price,
                self.config.grid_count,
            )
            .into_iter()
            .map(|p| market_info.round_price(p))
            .collect()
        };

        let num_zones = self.config.grid_count as usize - 1;
        let investment_per_zone = self.config.total_investment / num_zones as f64;

        if investment_per_zone < MIN_NOTIONAL_VALUE {
            let msg = format!(
                "Investment per zone ({:.2}) is less than minimum order value ({}). Increase total_investment or decrease grid_count.",
                investment_per_zone, MIN_NOTIONAL_VALUE
            );
            error!("[PERP_GRID] {}", msg);
            return Err(anyhow!(msg));
        }

        let initial_price = self.config.trigger_price.unwrap_or(last_price);

        // Validation: Check if wallet has enough margin
        let wallet_balance = ctx.get_perp_available("USDC");
        let max_notional = wallet_balance * self.config.leverage as f64;

        if max_notional < self.config.total_investment {
            let msg = format!(
                "Insufficient Margin! Balance: {:.2}, Lev: {}, Max Notional: {:.2}, Required: {:.2}. Bailing out.",
                wallet_balance, self.config.leverage, max_notional, self.config.total_investment
            );
            error!("[PERP_GRID] {}", msg);
            return Err(anyhow!(msg));
        }

        self.zones.clear();
        let mut total_position_required = 0.0;

        for i in 0..num_zones {
            let lower = prices[i];
            let upper = prices[i + 1];
            let mid_price = (lower + upper) / 2.0;

            let size = {
                let market_info = ctx.market_info(&self.config.symbol).unwrap();
                let raw_size = investment_per_zone / mid_price;
                market_info.clamp_to_min_notional(raw_size, mid_price, MIN_NOTIONAL_VALUE)
            };

            // Zone classification based on price line:
            // - Zone ABOVE price: lower > initial_price (both bounds > price)
            // - Zone BELOW price: upper < initial_price (both bounds < price)
            // - Zone CONTAINS price: lower <= initial_price <= upper
            let (pending_side, mode) = match self.config.grid_bias {
                GridBias::Long => {
                    // Long bias: acquire long positions above price, wait to open below
                    // Zone ABOVE price (lower > price): Have long → Sell to close
                    // Zone AT/BELOW price (lower <= price): No position → Buy to open
                    if lower > initial_price {
                        (OrderSide::Sell, ZoneMode::Long) // Zone above → close long
                    } else {
                        (OrderSide::Buy, ZoneMode::Long) // Zone at/below → open long
                    }
                }
                GridBias::Short => {
                    // Short bias: acquire short positions below price, wait to open above
                    // Zone BELOW price (upper < price): Have short → Buy to close
                    // Zone AT/ABOVE price (upper >= price): No position → Sell to open
                    if upper < initial_price {
                        (OrderSide::Buy, ZoneMode::Short) // Zone below → close short
                    } else {
                        (OrderSide::Sell, ZoneMode::Short) // Zone at/above → open short
                    }
                }
                GridBias::Neutral => {
                    // Neutral bias: use zone midpoint for classification
                    // Zone center above price → short mode
                    // Zone center at/below price → long mode
                    if mid_price > initial_price {
                        (OrderSide::Sell, ZoneMode::Short) // Zone above center → open short
                    } else {
                        (OrderSide::Buy, ZoneMode::Long) // Zone at/below center → open long
                    }
                }
            };

            if mode == ZoneMode::Long && pending_side.is_sell() {
                total_position_required += size;
            }
            if mode == ZoneMode::Short && pending_side.is_buy() {
                total_position_required -= size; // Short position needed (negative size)
            }

            self.zones.push(GridZone {
                index: i,
                lower_price: lower,
                upper_price: upper,
                size,
                pending_side,
                mode,
                entry_price: 0.0,
                order_id: None,
                roundtrip_count: 0,
            });
        }

        info!(
            "[PERP_GRID] Setup completed. Net position required: {}",
            total_position_required
        );

        if total_position_required.abs() > 0.0 {
            self.start_price = Some(initial_price);

            if let Some(trigger) = self.config.trigger_price {
                info!(
                    "[PERP_GRID] Assets required ({}), but waiting for trigger price {}",
                    total_position_required, trigger
                );
                self.state = StrategyState::WaitingForTrigger;
                return Ok(());
            }

            // Acquire Immediately
            info!(
                "[PERP_GRID] Acquiring initial position: {}",
                total_position_required
            );
            let cloid = ctx.generate_cloid();

            let (activation_price, target_size, side) = {
                let market_info = ctx.market_info(&self.config.symbol).unwrap();
                let market_price = market_info.last_price;
                let side = if total_position_required > 0.0 {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                };

                let raw_price = if side.is_buy() {
                    // Long Bias: Find highest grid level BELOW market
                    let grid_price = self
                        .zones
                        .iter()
                        .map(|z| z.lower_price)
                        .filter(|&p| p < market_price)
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(market_price);

                    // Cap spread at 0.1% (0.001)
                    // If grid level is too far (e.g. 1% away), bring it closer to 0.1% spread
                    let limit_price = market_price * (1.0 - 0.001);
                    grid_price.max(limit_price)
                } else {
                    // Short Bias: Find lowest grid level ABOVE market
                    let grid_price = self
                        .zones
                        .iter()
                        .map(|z| z.upper_price)
                        .filter(|&p| p > market_price)
                        .min_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(market_price);

                    // Cap spread at 0.1% (0.001)
                    let limit_price = market_price * (1.0 + 0.001);
                    grid_price.min(limit_price)
                };
                (
                    market_info.round_price(raw_price),
                    market_info.round_size(total_position_required.abs()),
                    side,
                )
            };

            if target_size > 0.0 {
                self.state = StrategyState::AcquiringAssets { cloid, target_size };
                info!(
                    "[ORDER_REQUEST] [PERP_GRID] REBALANCING: LIMIT {} {} {} @ {}",
                    side, target_size, self.config.symbol, activation_price
                );
                ctx.place_order(OrderRequest::Limit {
                    symbol: self.config.symbol.clone(),
                    side,
                    price: activation_price,
                    sz: target_size,
                    reduce_only: false,
                    cloid: Some(cloid),
                });
                return Ok(());
            }
        }

        self.start_price = Some(initial_price);
        self.state = StrategyState::Running;
        if let Err(e) = self.refresh_orders(ctx) {
            warn!("[PERP_GRID] Failed to refresh orders: {}", e);
        }
        Ok(())
    }

    fn refresh_orders(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        let market_info = match ctx.market_info(&self.config.symbol) {
            Some(i) => i.clone(),
            None => {
                error!("[PERP_GRID] No market info for {}", self.config.symbol);
                return Err(anyhow!("No market info for {}", self.config.symbol));
            }
        };

        for idx in 0..self.zones.len() {
            if self.zones[idx].order_id.is_none() {
                let (side, price, size, reduce_only) = {
                    let zone = &self.zones[idx];
                    let side = zone.pending_side;
                    let price = if side.is_buy() {
                        zone.lower_price
                    } else {
                        zone.upper_price
                    };
                    let reduce_only = match zone.mode {
                        ZoneMode::Short => side.is_buy(), // Buy closes short
                        ZoneMode::Long => side.is_sell(), // Sell closes long
                    };
                    (side, price, zone.size, reduce_only)
                };

                let cloid = ctx.generate_cloid();
                self.zones[idx].order_id = Some(cloid);
                self.active_orders.insert(cloid, idx);

                info!(
                    "[ORDER_REQUEST] [PERP_GRID] GRID_LVL_{}: LIMIT {} {} {} @ {}{}",
                    idx,
                    side,
                    size,
                    self.config.symbol,
                    price,
                    if reduce_only { " (RO)" } else { "" }
                );

                ctx.place_order(OrderRequest::Limit {
                    symbol: self.config.symbol.clone(),
                    side,
                    price: market_info.round_price(price),
                    sz: market_info.round_size(size),
                    reduce_only,
                    cloid: Some(cloid),
                });
            }
        }
        Ok(())
    }

    fn validate_fill_assertions(zone: &GridZone, fill: &OrderFill, zone_idx: usize) {
        // 1. Validate fill.side matches zone's pending order side
        let expected_side = zone.pending_side;
        if fill.side != expected_side {
            error!(
                "[PERP_GRID] ASSERTION FAILED: Zone {} expected side {:?} but got {:?}",
                zone_idx, expected_side, fill.side
            );
        }
        debug_assert_eq!(
            fill.side, expected_side,
            "Zone {} fill side mismatch: expected {:?}, got {:?}",
            zone_idx, expected_side, fill.side
        );

        // 2. Validate raw_dir matches expected exchange direction
        // Long mode: Buy = "Open Long", Sell = "Close Long"
        // Short mode: Sell = "Open Short", Buy = "Close Short"
        let expected_dir = match (zone.pending_side, zone.mode) {
            (OrderSide::Buy, ZoneMode::Long) => "Open Long",
            (OrderSide::Sell, ZoneMode::Long) => "Close Long",
            (OrderSide::Sell, ZoneMode::Short) => "Open Short",
            (OrderSide::Buy, ZoneMode::Short) => "Close Short",
        };
        if let Some(ref raw_dir) = fill.raw_dir {
            if raw_dir != expected_dir {
                error!(
                    "[PERP_GRID] ASSERTION FAILED: Zone {} expected raw_dir '{}' but got '{}'",
                    zone_idx, expected_dir, raw_dir
                );
            }
            debug_assert_eq!(
                raw_dir, expected_dir,
                "Zone {} raw_dir mismatch: expected '{}', got '{}'",
                zone_idx, expected_dir, raw_dir
            );
        }

        // 3. Validate reduce_only matches open/close expectation
        // Opening positions: reduce_only = false
        // Closing positions: reduce_only = true
        let expected_reduce_only = match (zone.pending_side, zone.mode) {
            (OrderSide::Buy, ZoneMode::Long) => false,   // Open Long
            (OrderSide::Sell, ZoneMode::Long) => true,   // Close Long
            (OrderSide::Sell, ZoneMode::Short) => false, // Open Short
            (OrderSide::Buy, ZoneMode::Short) => true,   // Close Short
        };
        if let Some(reduce_only) = fill.reduce_only {
            if reduce_only != expected_reduce_only {
                error!(
                    "[PERP_GRID] ASSERTION FAILED: Zone {} expected reduce_only={} but got {}",
                    zone_idx, expected_reduce_only, reduce_only
                );
            }
            debug_assert_eq!(
                reduce_only, expected_reduce_only,
                "Zone {} reduce_only mismatch: expected {}, got {}",
                zone_idx, expected_reduce_only, reduce_only
            );
        }
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

        let market_info = match ctx.market_info(&self.config.symbol) {
            Some(i) => i,
            None => {
                error!("[PERP_GRID] No market info for {}", self.config.symbol);
                return Err(anyhow!("No market info for {}", self.config.symbol));
            }
        };

        let (rounded_price, rounded_size) = (
            market_info.round_price(price),
            market_info.round_size(zone.size),
        );

        // Determine Reduce-Only for Counter Order
        // Counter order is the "closing" or "next step" order.
        // If we just filled Opening (WaitingBuy, LongBias), next is Closing (WaitingSell).
        // So reduce_only logic should be standard:
        // Long mode: Sell = Close (Reduce), Buy = Open.
        // Short mode: Buy = Close (Reduce), Sell = Open.
        let reduce_only = match zone.mode {
            ZoneMode::Short => side.is_buy(), // Buying to close short
            ZoneMode::Long => side.is_sell(), // Selling to close long
        };

        info!(
            "[ORDER_REQUEST] [PERP_GRID] COUNTER_ORDER: LIMIT {} {} {} @ {}{}",
            side,
            rounded_size,
            self.config.symbol,
            rounded_price,
            if reduce_only { " (RO)" } else { "" }
        );

        self.active_orders.insert(next_cloid, zone_idx);
        zone.order_id = Some(next_cloid);

        ctx.place_order(OrderRequest::Limit {
            symbol: self.config.symbol.clone(),
            side,
            price: rounded_price,
            sz: rounded_size,
            reduce_only,
            cloid: Some(next_cloid),
        });

        Ok(())
    }
}

impl Strategy for PerpGridStrategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()> {
        match self.state {
            StrategyState::Initializing => {
                if let Some(market_info) = ctx.market_info(&self.config.symbol) {
                    if market_info.last_price > 0.0 {
                        self.initialize_zones(ctx)?;
                    }
                }
            }
            StrategyState::WaitingForTrigger => {
                if let Some(trigger) = self.config.trigger_price {
                    // Need start_price to know direction??
                    // initialize_zones sets self.start_price
                    let start = self
                        .start_price
                        .expect("Start price must be set when in WaitingForTrigger state");

                    if common::check_trigger(price, trigger, start) {
                        info!(
                            "[PERP_GRID] Price {} crossed trigger {}. Starting.",
                            price, trigger
                        );

                        info!("[PERP_GRID] Triggered! Re-initializing zones for accurate state.");
                        self.zones.clear();
                        self.initialize_zones(ctx)?;
                    }
                } else {
                    self.state = StrategyState::Running;
                }
            }
            StrategyState::AcquiringAssets { .. } => {
                // Asset acquisition is handled via order fills.
                // We just wait here.
                // Maybe check for timeout? Feature for later.
            }
            StrategyState::Running => {
                self.refresh_orders(ctx)
                    .unwrap_or_else(|e| warn!("[PERP_GRID] Failed refresh: {}", e));
            }
        }
        Ok(())
    }

    fn on_order_filled(&mut self, fill: &OrderFill, ctx: &mut StrategyContext) -> Result<()> {
        if let Some(cloid_val) = fill.cloid {
            // Check for Acquisition Fill
            if let StrategyState::AcquiringAssets {
                cloid: acq_cloid, ..
            } = self.state
            {
                if cloid_val == acq_cloid {
                    info!("[PERP_GRID] Acquisition filled @ {}", fill.price);

                    // Track fees
                    self.total_fees += fill.fee;

                    // Update Position Size and Average Entry Price
                    let old_pos = self.position_size;
                    if fill.side.is_buy() {
                        self.position_size += fill.size;
                        // Update weighted average entry (opening long)
                        if self.position_size.abs() > 0.0 {
                            self.avg_entry_price = (old_pos.abs() * self.avg_entry_price
                                + fill.size * fill.price)
                                / self.position_size.abs();
                        }
                    } else {
                        self.position_size -= fill.size;
                        // Update weighted average entry (opening short)
                        if self.position_size.abs() > 0.0 {
                            self.avg_entry_price = (old_pos.abs() * self.avg_entry_price
                                + fill.size * fill.price)
                                / self.position_size.abs();
                        }
                    }

                    // Update Zones Entry Price
                    for zone in &mut self.zones {
                        // Long mode: We bought, now waiting to Sell (Close Long). Set entry.
                        if zone.mode == ZoneMode::Long && zone.pending_side.is_sell() {
                            zone.entry_price = fill.price;
                        }
                        // Short mode: We sold, now waiting to Buy (Close Short). Set entry.
                        if zone.mode == ZoneMode::Short && zone.pending_side.is_buy() {
                            zone.entry_price = fill.price;
                        }
                    }

                    self.start_price = Some(fill.price);
                    self.state = StrategyState::Running;
                    self.refresh_orders(ctx)?;
                    return Ok(());
                }
            }

            if let Some(zone_idx) = self.active_orders.remove(&cloid_val) {
                self.trade_count += 1;
                self.total_fees += fill.fee;

                let (next_px, next_side, pnl) = {
                    let zone = &mut self.zones[zone_idx];
                    zone.order_id = None;

                    // Validate fill assertions
                    Self::validate_fill_assertions(zone, fill, zone_idx);

                    // Determine if this is an opening or closing fill
                    let is_opening = match (zone.pending_side, zone.mode) {
                        (OrderSide::Buy, ZoneMode::Long) => true,   // Open Long
                        (OrderSide::Sell, ZoneMode::Short) => true, // Open Short
                        _ => false,                                 // Closing
                    };

                    // Update Position Size and Average Entry Price
                    let old_pos = self.position_size;
                    if zone.pending_side.is_buy() {
                        self.position_size += fill.size;
                    } else {
                        self.position_size -= fill.size;
                    }

                    // Update avg_entry_price only for opening fills
                    if is_opening && self.position_size.abs() > 0.0 {
                        self.avg_entry_price = (old_pos.abs() * self.avg_entry_price
                            + fill.size * fill.price)
                            / self.position_size.abs();
                    } else if self.position_size.abs() < 0.0001 {
                        // Position closed, reset avg_entry
                        self.avg_entry_price = 0.0;
                    }

                    let (next_side, entry_px, pnl, next_px) = match (zone.pending_side, zone.mode) {
                        (OrderSide::Buy, ZoneMode::Long) => {
                            info!(
                                "[PERP_GRID] Zone {} | BUY (Open Long) Filled @ {} | Size: {} | Next: SELL (Close) @ {}",
                                zone_idx, fill.price, fill.size, zone.upper_price
                            );
                            (OrderSide::Sell, fill.price, None, zone.upper_price)
                        }
                        (OrderSide::Sell, ZoneMode::Long) => {
                            let pnl = (fill.price - zone.entry_price) * fill.size;
                            zone.roundtrip_count += 1;
                            info!(
                                "[PERP_GRID] Zone {} | SELL (Close Long) Filled @ {} | PnL: {:.4} | Next: BUY (Open) @ {}",
                                zone_idx, fill.price, pnl, zone.lower_price
                            );
                            (OrderSide::Buy, 0.0, Some(pnl), zone.lower_price)
                        }
                        (OrderSide::Sell, ZoneMode::Short) => {
                            info!(
                                "[PERP_GRID] Zone {} | SELL (Open Short) Filled @ {} | Size: {} | Next: BUY (Close) @ {}",
                                zone_idx, fill.price, fill.size, zone.lower_price
                            );
                            (OrderSide::Buy, fill.price, None, zone.lower_price)
                        }
                        (OrderSide::Buy, ZoneMode::Short) => {
                            let pnl = (zone.entry_price - fill.price) * fill.size;
                            zone.roundtrip_count += 1;
                            info!(
                                "[PERP_GRID] Zone {} | BUY (Close Short) Filled @ {} | PnL: {:.4} | Next: SELL (Open) @ {}",
                                zone_idx, fill.price, pnl, zone.upper_price
                            );
                            (OrderSide::Sell, 0.0, Some(pnl), zone.upper_price)
                        }
                    };

                    zone.pending_side = next_side;
                    zone.entry_price = entry_px;
                    (next_px, next_side, pnl)
                };

                // Accumulate realized PnL from closing fills
                if let Some(pnl) = pnl {
                    self.realized_pnl += pnl;
                }

                self.place_counter_order(zone_idx, next_px, next_side, ctx)?;
            } else {
                debug!(
                    "[PERP_GRID] Fill received for unknown/inactive Perp CLOID: {}",
                    cloid_val
                );
            }
        } else {
            debug!(
                "[PERP_GRID] Fill received without CLOID in PerpStrategy at price {}",
                fill.price
            );
        }
        Ok(())
    }

    fn on_order_failed(&mut self, _cloid: Cloid, _ctx: &mut StrategyContext) -> Result<()> {
        Ok(())
    }

    fn get_summary(&self, ctx: &StrategyContext) -> StrategySummary {
        use crate::broadcast::types::PerpGridSummary;

        let current_price = ctx
            .market_info(&self.config.symbol)
            .map(|m| m.last_price)
            .unwrap_or(0.0);

        // Calculate total roundtrips from zones
        let total_roundtrips: u32 = self.zones.iter().map(|z| z.roundtrip_count).sum();

        // Determine position side and calculate unrealized PnL
        let (position_side, unrealized_pnl) = if self.position_size > 0.0 {
            // Long position: profit when price goes up
            let pnl = (current_price - self.avg_entry_price) * self.position_size;
            ("Long", pnl)
        } else if self.position_size < 0.0 {
            // Short position: profit when price goes down
            let pnl = (self.avg_entry_price - current_price) * self.position_size.abs();
            ("Short", pnl)
        } else {
            ("Flat", 0.0)
        };

        // Calculate grid spacing percentage
        let grid_spacing_pct = common::calculate_grid_spacing_pct(
            &self.config.grid_type,
            self.config.lower_price,
            self.config.upper_price,
            self.config.grid_count,
        );

        // Calculate uptime
        let uptime = common::format_uptime(self.start_time.elapsed());

        StrategySummary::PerpGrid(PerpGridSummary {
            symbol: self.config.symbol.clone(),
            price: current_price,
            state: format!("{:?}", self.state),
            uptime,
            position_size: self.position_size,
            position_side: position_side.to_string(),
            avg_entry_price: self.avg_entry_price,
            realized_pnl: self.realized_pnl,
            unrealized_pnl,
            total_fees: self.total_fees,
            leverage: self.config.leverage,
            grid_bias: format!("{:?}", self.config.grid_bias),
            grid_count: self.zones.len() as u32,
            range_low: self.config.lower_price,
            range_high: self.config.upper_price,
            grid_spacing_pct,
            roundtrips: total_roundtrips,
            margin_balance: ctx.get_perp_available("USDC"),
            start_price: self.start_price,
        })
    }

    fn get_grid_state(&self, ctx: &StrategyContext) -> GridState {
        use crate::broadcast::types::ZoneInfo;

        let current_price = ctx
            .market_info(&self.config.symbol)
            .map(|m| m.last_price)
            .unwrap_or(0.0);

        let zones = self
            .zones
            .iter()
            .map(|z| {
                // Frontend will derive labels. We just need reduce_only for the struct.
                let is_reduce_only = match z.mode {
                    ZoneMode::Short => z.pending_side.is_buy(),
                    ZoneMode::Long => z.pending_side.is_sell(),
                };

                ZoneInfo {
                    index: z.index,
                    lower_price: z.lower_price,
                    upper_price: z.upper_price,
                    size: z.size,
                    pending_side: z.pending_side.to_string(),
                    has_order: z.order_id.is_some(),
                    is_reduce_only,
                    entry_price: z.entry_price,
                    roundtrip_count: z.roundtrip_count,
                }
            })
            .collect();

        GridState {
            symbol: self.config.symbol.clone(),
            strategy_type: "perp_grid".to_string(),
            current_price,
            grid_bias: Some(format!("{:?}", self.config.grid_bias)),
            zones,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::strategy::PerpGridConfig;
    use crate::engine::context::{MarketInfo, StrategyContext};
    use crate::strategy::types::GridType;
    use std::collections::HashMap;

    fn create_test_context(symbol: &str) -> StrategyContext {
        let mut markets = HashMap::new();
        markets.insert(
            symbol.to_string(),
            MarketInfo::new(symbol.to_string(), "HYPE".to_string(), 0, 2, 2),
        );
        let mut ctx = StrategyContext::new(markets);
        if let Some(info) = ctx.market_info_mut(symbol) {
            info.last_price = 100.0;
        }
        ctx
    }

    #[test]
    fn test_perp_grid_init_long_bias() {
        let symbol = "HYPE".to_string();
        let mut ctx = create_test_context(&symbol);

        // No balances needed for pure state logic, but typically we set them
        ctx.update_perp_balance("USDC".to_string(), 10000.0, 10000.0);

        let config = PerpGridConfig {
            symbol: symbol.clone(),
            leverage: 10,
            is_isolated: true,
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 3, // 90, 100, 110 -> zones: [90-100], [100-110]
            total_investment: 1000.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);

        // Set last_price to 99 so zones are classified correctly
        if let Some(info) = ctx.market_info_mut(&symbol) {
            info.last_price = 99.0;
        }
        // Use price 99 (inside zone [90-100], below zone [100-110])
        strategy.on_tick(99.0, &mut ctx).unwrap();

        assert_eq!(strategy.zones.len(), 2);
        // Zone 0 [90-100]: lower=90, 99 < 90 = false → Buy (zone at/below price)
        assert_eq!(strategy.zones[0].pending_side, OrderSide::Buy);
        // Zone 1 [100-110]: lower=100, 99 < 100 = true → Sell (zone above price)
        assert_eq!(strategy.zones[1].pending_side, OrderSide::Sell);

        match strategy.state {
            StrategyState::AcquiringAssets { target_size, .. } => {
                assert!(target_size > 0.0);
            }
            _ => panic!("Should be acquiring assets, got {:?}", strategy.state),
        }
    }

    #[test]
    fn test_perp_grid_execution_flow() {
        let symbol = "HYPE".to_string();
        let mut ctx = create_test_context(&symbol);
        ctx.update_perp_balance("USDC".to_string(), 1000.0, 1000.0);

        let config = PerpGridConfig {
            symbol: symbol.clone(),
            leverage: 1,
            is_isolated: true,
            upper_price: 120.0,
            lower_price: 80.0,
            grid_type: GridType::Arithmetic,
            grid_count: 3, // 80, 100, 120 -> zones: [80-100], [100-120]
            total_investment: 100.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);

        // Set last_price to 99 so zone [100-120] is above price (Sell)
        if let Some(info) = ctx.market_info_mut(&symbol) {
            info.last_price = 99.0;
        }
        strategy.on_tick(99.0, &mut ctx).unwrap();

        let cloid = match strategy.state {
            StrategyState::AcquiringAssets { cloid, .. } => cloid,
            _ => panic!("Init failed, state: {:?}", strategy.state),
        };

        // Clear acquisition order from queue as it's "sent"
        ctx.order_queue.clear();

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: 0.5,
                    price: 100.0,
                    fee: 0.0,
                    cloid: Some(cloid),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();
        assert!(matches!(strategy.state, StrategyState::Running));

        // Verify orders in queue
        let orders = &ctx.order_queue;
        assert_eq!(orders.len(), 2);

        // Note: StrategyContext::place_limit_order pushes to order_queue.
        // We check valid placement logic.

        // Find Sell (Reduce Only)
        let buy_orders: Vec<_> = orders
            .iter()
            .filter(|o| match o {
                crate::model::OrderRequest::Limit { side, .. } => side.is_buy(),
                crate::model::OrderRequest::Market { side, .. } => side.is_buy(),
                _ => false,
            })
            .collect();

        let sell_orders: Vec<_> = orders
            .iter()
            .filter(|o| match o {
                crate::model::OrderRequest::Limit { side, .. } => side.is_sell(),
                crate::model::OrderRequest::Market { side, .. } => side.is_sell(),
                _ => false,
            })
            .collect();

        // With simplified checks
        assert_eq!(sell_orders.len(), 1);

        let sell = sell_orders[0];
        // match OrderRequest::Limit
        match sell {
            crate::model::OrderRequest::Limit {
                price, reduce_only, ..
            } => {
                assert_eq!(*price, 120.0);
                assert!(*reduce_only);
            }
            _ => panic!("Expected Limit Order"),
        }

        let buy = buy_orders[0];
        match buy {
            crate::model::OrderRequest::Limit {
                price, reduce_only, ..
            } => {
                assert_eq!(*price, 80.0);
                assert!(!*reduce_only);
            }
            _ => panic!("Expected Limit Order"),
        }
    }

    #[test]
    fn test_perp_grid_inventory_tracking() {
        let symbol = "HYPE".to_string();
        let mut ctx = create_test_context(&symbol);
        ctx.update_perp_balance("USDC".to_string(), 1000.0, 1000.0);

        let config = PerpGridConfig {
            symbol: symbol.clone(),
            leverage: 1,
            is_isolated: true,
            upper_price: 120.0,
            lower_price: 80.0,
            grid_type: GridType::Arithmetic,
            grid_count: 3, // 80, 100, 120 -> zones: [80-100], [100-120]
            total_investment: 100.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);

        // Set last_price to 99 so zone [100-120] is above price (Sell)
        if let Some(info) = ctx.market_info_mut(&symbol) {
            info.last_price = 99.0;
        }
        strategy.on_tick(99.0, &mut ctx).unwrap();

        // 1. Initial State: Empty position
        assert_eq!(strategy.position_size, 0.0);

        // 2. Acquisition Fill (if any)
        // With current setup (Long Bias, Price 99, Range 80-120), zones are:
        // Zone [80-100]: lower=80, 99 < 80 = false → Buy (open long)
        // Zone [100-120]: lower=100, 99 < 100 = true → Sell (close long, need position first)
        // It enters AcquiringAssets.

        if let StrategyState::AcquiringAssets { cloid, target_size } = strategy.state {
            assert!(target_size > 0.0);

            // Fill Acquisition
            strategy
                .on_order_filled(
                    &OrderFill {
                        side: OrderSide::Buy,
                        size: target_size,
                        price: 100.0,
                        fee: 0.0,
                        cloid: Some(cloid),
                        reduce_only: None,
                        raw_dir: None,
                    },
                    &mut ctx,
                )
                .unwrap();

            // Check inventory updated
            assert_eq!(strategy.position_size, target_size);
        } else {
            panic!("Expected AcquiringAssets state");
        }

        // 3. Grid Trading Fills
        // Now Running.
        // Zone 0 (80-100) is WaitingBuy.
        // Zone 1 (100-120) is WaitingSell.

        // Find Active Order for Zone 0 (Buy @ 80)
        let zone0_idx = 0;
        let zone0 = &strategy.zones[zone0_idx];
        assert_eq!(zone0.pending_side, OrderSide::Buy);
        let order_id = zone0.order_id.expect("Zone 0 should have order");

        let size = zone0.size;

        // Simulate Fill: Buy @ 80
        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size,
                    price: 80.0,
                    fee: 0.0,
                    cloid: Some(order_id),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        // Expect inventory increase (Long Bias + Buy = Add to Position)
        // Previous position was `target_size` (from acquisition).
        // Now should be `target_size + size`.
        let expected_size = strategy.zones[1].size + size; // Approx
        assert!(
            (strategy.position_size - expected_size).abs() < 0.0001,
            "Position size mismatch after BUY"
        );

        // Zone 0 flips to WaitingSell (Close Long). placing Sell @ 100.
        let zone0 = &strategy.zones[zone0_idx];
        assert_eq!(zone0.pending_side, OrderSide::Sell);
        let sell_oid = zone0.order_id.expect("Zone 0 should have new sell order");

        // Simulate Fill: Sell @ 100
        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Sell,
                    size,
                    price: 100.0,
                    fee: 0.0,
                    cloid: Some(sell_oid),
                    reduce_only: None,
                    raw_dir: None,
                },
                &mut ctx,
            )
            .unwrap();

        // Expect inventory decrease
        // Back to initial acquisition size
        let expected_size_after_sell = strategy.zones[1].size;
        assert!(
            (strategy.position_size - expected_size_after_sell).abs() < 0.0001,
            "Position size mismatch after SELL"
        );
    }

    #[test]
    fn test_perp_grid_pnl_and_ping_pong() {
        // Test full ping-pong cycle with:
        // 1. reduce_only flags set correctly
        // 2. raw_dir matching exchange expectations
        // 3. PnL calculation on closing fills
        // 4. Fee accumulation
        // 5. Counter order generation (ping-pong)

        let symbol = "TEST".to_string();
        let mut ctx = create_test_context(&symbol);
        ctx.update_perp_balance("USDC".to_string(), 10000.0, 10000.0);

        let config = PerpGridConfig {
            symbol: symbol.clone(),
            leverage: 1,
            is_isolated: true,
            upper_price: 120.0,
            lower_price: 80.0,
            grid_type: GridType::Arithmetic,
            grid_count: 3,
            total_investment: 1000.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);

        // Set last_price to 95 so zones are classified correctly
        if let Some(info) = ctx.market_info_mut(&symbol) {
            info.last_price = 95.0;
        }

        // Initialize at price 95
        // Zone 0 [90-100]: lower=90, 95 < 90 = false → Buy (open long, no position yet)
        // Zone 1 [100-110]: lower=100, 95 < 100 = true → Sell (close long, need to acquire)
        strategy.on_tick(95.0, &mut ctx).unwrap();

        // Verify initial state
        assert_eq!(strategy.realized_pnl, 0.0);
        assert_eq!(strategy.total_fees, 0.0);
        assert_eq!(strategy.position_size, 0.0);

        // Complete acquisition if needed
        if let StrategyState::AcquiringAssets { cloid, target_size } = strategy.state {
            let acq_fee = 0.5; // $0.50 fee

            strategy
                .on_order_filled(
                    &OrderFill {
                        side: OrderSide::Buy,
                        size: target_size,
                        price: 95.0,
                        fee: acq_fee,
                        cloid: Some(cloid),
                        reduce_only: Some(false), // Opening position
                        raw_dir: Some("Open Long".to_string()),
                    },
                    &mut ctx,
                )
                .unwrap();

            // Verify fee tracked
            assert_eq!(strategy.total_fees, acq_fee);
            // Verify avg_entry set
            assert!((strategy.avg_entry_price - 95.0).abs() < 0.01);
        }

        assert!(matches!(strategy.state, StrategyState::Running));
        ctx.order_queue.clear();

        // Find a zone in WaitingBuy state (for Long bias, this is the opening order)
        let buy_zone_idx = strategy
            .zones
            .iter()
            .position(|z| z.pending_side.is_buy())
            .expect("Should have a WaitingBuy zone");

        let zone = &strategy.zones[buy_zone_idx];
        let buy_cloid = zone.order_id.expect("Zone should have order");
        let zone_size = zone.size;
        let buy_price = zone.lower_price; // Buy at lower bound

        // ============================================================
        // STEP 1: Open Long (Buy) - reduce_only = false
        // ============================================================
        let buy_fee = 0.25;
        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: zone_size,
                    price: buy_price,
                    fee: buy_fee,
                    cloid: Some(buy_cloid),
                    reduce_only: Some(false), // Opening position
                    raw_dir: Some("Open Long".to_string()),
                },
                &mut ctx,
            )
            .unwrap();

        // Verify: No PnL on opening fill
        assert_eq!(strategy.realized_pnl, 0.0);
        // Verify: Fees accumulated
        assert!((strategy.total_fees - 0.75).abs() < 0.01); // 0.5 + 0.25
                                                            // Verify: Zone flipped to WaitingSell
        assert_eq!(strategy.zones[buy_zone_idx].pending_side, OrderSide::Sell);
        // Verify: Entry price recorded
        assert!((strategy.zones[buy_zone_idx].entry_price - buy_price).abs() < 0.01);
        // Verify: Counter order placed (Sell at upper price)
        let sell_cloid = strategy.zones[buy_zone_idx]
            .order_id
            .expect("Should have new sell order");

        // Check the counter order in queue
        let sell_order = ctx.order_queue.last().expect("Should have order in queue");
        match sell_order {
            OrderRequest::Limit {
                side,
                price,
                reduce_only,
                ..
            } => {
                assert!(side.is_sell(), "Counter order should be Sell");
                assert!(
                    (*price - strategy.zones[buy_zone_idx].upper_price).abs() < 0.01,
                    "Sell should be at upper_price"
                );
                assert!(*reduce_only, "Close Long should be reduce_only");
            }
            _ => panic!("Expected Limit order"),
        }

        ctx.order_queue.clear();

        // ============================================================
        // STEP 2: Close Long (Sell) - reduce_only = true, generates PnL
        // ============================================================
        let sell_price = strategy.zones[buy_zone_idx].upper_price; // Sell at upper bound
        let sell_fee = 0.30;
        let expected_pnl = (sell_price - buy_price) * zone_size; // Profit!

        let roundtrips_before = strategy.zones[buy_zone_idx].roundtrip_count;

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Sell,
                    size: zone_size,
                    price: sell_price,
                    fee: sell_fee,
                    cloid: Some(sell_cloid),
                    reduce_only: Some(true), // Closing position!
                    raw_dir: Some("Close Long".to_string()),
                },
                &mut ctx,
            )
            .unwrap();

        // Verify: PnL calculated correctly
        assert!(
            (strategy.realized_pnl - expected_pnl).abs() < 0.01,
            "Expected PnL {:.4}, got {:.4}",
            expected_pnl,
            strategy.realized_pnl
        );

        // Verify: Fees accumulated
        assert!((strategy.total_fees - 1.05).abs() < 0.01); // 0.5 + 0.25 + 0.30

        // Verify: Roundtrip count incremented
        assert_eq!(
            strategy.zones[buy_zone_idx].roundtrip_count,
            roundtrips_before + 1
        );

        // Verify: Zone flipped back to WaitingBuy (ping-pong!)
        assert_eq!(strategy.zones[buy_zone_idx].pending_side, OrderSide::Buy);

        // Verify: Entry price reset
        assert_eq!(strategy.zones[buy_zone_idx].entry_price, 0.0);

        // Verify: New buy order placed (ping-pong counter order)
        let new_buy_order = ctx.order_queue.last().expect("Should have order in queue");
        match new_buy_order {
            OrderRequest::Limit {
                side,
                price,
                reduce_only,
                ..
            } => {
                assert!(side.is_buy(), "Counter order should be Buy");
                assert!(
                    (*price - strategy.zones[buy_zone_idx].lower_price).abs() < 0.01,
                    "Buy should be at lower_price"
                );
                assert!(!*reduce_only, "Open Long should NOT be reduce_only");
            }
            _ => panic!("Expected Limit order"),
        }

        println!(
            "✅ Ping-Pong Complete! PnL: {:.4}, Total Fees: {:.4}, Roundtrips: {}",
            strategy.realized_pnl,
            strategy.total_fees,
            strategy.zones[buy_zone_idx].roundtrip_count
        );
    }

    #[test]
    fn test_perp_grid_short_bias_pnl() {
        // Test Short bias: Sell high, buy low
        let symbol = "TEST".to_string();
        let mut ctx = create_test_context(&symbol);
        ctx.update_perp_balance("USDC".to_string(), 10000.0, 10000.0);

        let config = PerpGridConfig {
            symbol: symbol.clone(),
            leverage: 1,
            is_isolated: true,
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 3,
            total_investment: 1000.0,
            grid_bias: GridBias::Short, // Short bias!
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);

        // Set last_price to 105 so Zone 0 is below price (Buy/close short)
        // Zone 0 [90-100]: upper=100 < 105 → Buy (close short, acquired position)
        // Zone 1 [100-110]: upper=110 < 105 → false → Sell (open short)
        if let Some(info) = ctx.market_info_mut(&symbol) {
            info.last_price = 105.0;
        }
        strategy.on_tick(105.0, &mut ctx).unwrap();

        // Handle acquisition for short bias
        if let StrategyState::AcquiringAssets { cloid, target_size } = strategy.state {
            strategy
                .on_order_filled(
                    &OrderFill {
                        side: OrderSide::Sell,
                        size: target_size,
                        price: 105.0, // Acquisition at current price
                        fee: 0.5,
                        cloid: Some(cloid),
                        reduce_only: Some(false),
                        raw_dir: Some("Open Short".to_string()),
                    },
                    &mut ctx,
                )
                .unwrap();

            // Position should be negative (short)
            assert!(strategy.position_size < 0.0);
        }

        ctx.order_queue.clear();

        let close_zone_idx = strategy
            .zones
            .iter()
            .position(|z| z.pending_side.is_buy() && z.mode == ZoneMode::Short)
            .expect("Should have a WaitingBuy zone for short bias");

        let zone = &strategy.zones[close_zone_idx];
        let cloid = zone.order_id.expect("Zone should have order");
        let zone_size = zone.size;
        let entry_price = zone.entry_price; // Entry from acquisition
        let close_price = zone.lower_price; // Buy at lower to close short

        // Close Short (Buy) - reduce_only = true, generates PnL
        let expected_pnl = (entry_price - close_price) * zone_size; // Profit when close < entry

        strategy
            .on_order_filled(
                &OrderFill {
                    side: OrderSide::Buy,
                    size: zone_size,
                    price: close_price,
                    fee: 0.25,
                    cloid: Some(cloid),
                    reduce_only: Some(true), // Closing short position
                    raw_dir: Some("Close Short".to_string()),
                },
                &mut ctx,
            )
            .unwrap();

        // Verify PnL for short: profit = (entry - close) * size
        assert!(
            (strategy.realized_pnl - expected_pnl).abs() < 0.01,
            "Short PnL: expected {:.4}, got {:.4}",
            expected_pnl,
            strategy.realized_pnl
        );

        // Verify zone flipped to WaitingSell (ping-pong: now open short again)
        assert_eq!(strategy.zones[close_zone_idx].pending_side, OrderSide::Sell);

        // Verify counter order is Sell (Open Short)
        let counter_order = ctx.order_queue.last().expect("Should have order");
        match counter_order {
            OrderRequest::Limit {
                side, reduce_only, ..
            } => {
                assert!(side.is_sell(), "Counter should be Sell (Open Short)");
                assert!(!*reduce_only, "Open Short should NOT be reduce_only");
            }
            _ => panic!("Expected Limit order"),
        }

        println!(
            "✅ Short Bias Ping-Pong Complete! PnL: {:.4}",
            strategy.realized_pnl
        );
    }

    /// Test to reproduce the Long bias zone classification bug.
    ///
    /// Grid: 90-110, 4 zones: [90-95], [95-100], [100-105], [105-110]
    /// Price: 105 (exactly at zone boundary)
    ///
    /// Expected (zones truly ABOVE price line have lower > price):
    /// - Zone [90-95]:   lower=90  < 105 → Buy (open long)
    /// - Zone [95-100]:  lower=95  < 105 → Buy (open long)
    /// - Zone [100-105]: lower=100 < 105 → Buy (open long) ← Contains price
    /// - Zone [105-110]: lower=105 = 105 → Buy (at boundary, not above) ← EDGE CASE
    ///
    /// Current buggy behavior (uses upper):
    /// - Zone [100-105]: 105 < 105? false → Buy ✓
    /// - Zone [105-110]: 105 < 110? true → Sell ✗ (WRONG!)
    #[test]
    fn test_long_bias_zone_classification_at_boundary() {
        let mut markets = HashMap::new();
        markets.insert(
            "BTC".to_string(),
            crate::engine::context::MarketInfo::new("BTC".to_string(), "BTC".to_string(), 0, 5, 0),
        );
        let mut ctx = StrategyContext::new(markets);
        ctx.update_perp_balance("USDC".to_string(), 10000.0, 10000.0);

        if let Some(info) = ctx.market_info_mut("BTC") {
            info.last_price = 105.0;
        }

        let config = PerpGridConfig {
            symbol: "BTC".to_string(),
            leverage: 1,
            is_isolated: false,
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5, // 4 zones: [90-95], [95-100], [100-105], [105-110]
            total_investment: 1000.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);

        // Initialize at price 105 (exactly at zone [105-110] lower boundary)
        strategy.on_tick(105.0, &mut ctx).unwrap();

        // Print zone states for debugging
        println!("Long Bias | Price: 105.0");
        println!("Zone classification (current behavior):");
        for zone in &strategy.zones {
            println!(
                "  Zone {} [{}-{}]: pending={:?}, mode={:?}",
                zone.index, zone.lower_price, zone.upper_price, zone.pending_side, zone.mode
            );
        }

        // Zone [105-110]: lower=105, price=105
        // This zone's lower boundary EQUALS the price
        // Per user's logic: zone is NOT above (lower > price fails: 105 > 105 = false)
        // So it should be Buy (waiting to open long), not Sell
        let zone_at_boundary = &strategy.zones[3]; // [105-110]
        assert_eq!(zone_at_boundary.lower_price, 105.0);
        assert_eq!(zone_at_boundary.upper_price, 110.0);

        // BUG: Current code classifies this as Sell because 105 < 110
        // EXPECTED: Should be Buy because 105 is NOT < 105 (lower)
        println!("\n🔍 Zone [105-110] at price boundary:");
        println!(
            "  Current: pending_side = {:?}",
            zone_at_boundary.pending_side
        );
        println!("  Expected: pending_side = Buy (zone at boundary, not above)");

        // This assertion will FAIL with current buggy code,
        // demonstrating the bug:
        assert_eq!(
            zone_at_boundary.pending_side,
            OrderSide::Buy,
            "Zone [105-110] at price boundary should be Buy, not Sell"
        );
    }

    /// Test to reproduce the Short bias zone classification bug.
    ///
    /// Grid: 90-110, 4 zones: [90-95], [95-100], [100-105], [105-110]
    /// Price: 95 (exactly at zone boundary)
    ///
    /// Expected (zones truly BELOW price line have upper < price):
    /// - Zone [90-95]:   upper=95  = 95 → Sell (at boundary, not below) ← EDGE CASE
    /// - Zone [95-100]:  upper=100 > 95 → Sell (open short)
    /// - Zone [100-105]: upper=105 > 95 → Sell (open short)
    /// - Zone [105-110]: upper=110 > 95 → Sell (open short)
    ///
    /// Current buggy behavior (uses lower):
    /// - Zone [90-95]: 95 > 90? true → Buy ✗ (WRONG!)
    #[test]
    fn test_short_bias_zone_classification_at_boundary() {
        let mut markets = HashMap::new();
        markets.insert(
            "BTC".to_string(),
            crate::engine::context::MarketInfo::new("BTC".to_string(), "BTC".to_string(), 0, 5, 0),
        );
        let mut ctx = StrategyContext::new(markets);
        ctx.update_perp_balance("USDC".to_string(), 10000.0, 10000.0);

        if let Some(info) = ctx.market_info_mut("BTC") {
            info.last_price = 95.0;
        }

        let config = PerpGridConfig {
            symbol: "BTC".to_string(),
            leverage: 1,
            is_isolated: false,
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 5, // 4 zones: [90-95], [95-100], [100-105], [105-110]
            total_investment: 1000.0,
            grid_bias: GridBias::Short,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);

        // Initialize at price 95 (exactly at zone [90-95] upper boundary)
        strategy.on_tick(95.0, &mut ctx).unwrap();

        // Print zone states for debugging
        println!("Short Bias | Price: 95.0");
        println!("Zone classification (current behavior):");
        for zone in &strategy.zones {
            println!(
                "  Zone {} [{}-{}]: pending={:?}, mode={:?}",
                zone.index, zone.lower_price, zone.upper_price, zone.pending_side, zone.mode
            );
        }

        // Zone [90-95]: upper=95, price=95
        // This zone's upper boundary EQUALS the price
        // Per user's logic: zone is NOT below (upper < price fails: 95 < 95 = false)
        // So it should be Sell (waiting to open short), not Buy
        let zone_at_boundary = &strategy.zones[0]; // [90-95]
        assert_eq!(zone_at_boundary.lower_price, 90.0);
        assert_eq!(zone_at_boundary.upper_price, 95.0);

        // BUG: Current code classifies this as Buy because 95 > 90
        // EXPECTED: Should be Sell because 95 is NOT > 95 (upper)
        println!("\n🔍 Zone [90-95] at price boundary:");
        println!(
            "  Current: pending_side = {:?}",
            zone_at_boundary.pending_side
        );
        println!("  Expected: pending_side = Sell (zone at boundary, not below)");

        // This assertion will FAIL with current buggy code,
        // demonstrating the bug:
        assert_eq!(
            zone_at_boundary.pending_side,
            OrderSide::Sell,
            "Zone [90-95] at price boundary should be Sell, not Buy"
        );
    }
}
