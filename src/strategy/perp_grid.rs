use crate::config::strategy::StrategyConfig;
use crate::engine::context::{StrategyContext, MIN_NOTIONAL_VALUE};
use crate::model::{Cloid, OrderFill, OrderRequest, OrderSide};
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::common;
use super::types::{GridBias, GridType, ZoneState};

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
    state: ZoneState,
    is_short_oriented: bool,
    entry_price: f64,
    order_id: Option<Cloid>,

    // Performance Metrics
    roundtrip_count: u32,
}

#[allow(dead_code)]
pub struct PerpGridStrategy {
    symbol: String,
    leverage: u32,
    is_isolated: bool,
    upper_price: f64,
    lower_price: f64,
    grid_type: GridType,
    grid_count: u32,
    total_investment: f64,
    grid_bias: GridBias,
    trigger_price: Option<f64>,

    // Internal State
    zones: Vec<GridZone>,
    active_orders: HashMap<Cloid, usize>, // cloid -> zone_index
    trade_count: u32,
    state: StrategyState,
    start_price: Option<f64>,

    // Performance Metrics
    realized_pnl: f64,
    total_fees: f64,
    unrealized_pnl: f64,

    // Position Tracking
    position_size: f64,
    avg_entry_price: f64,
}

impl PerpGridStrategy {
    pub fn new(config: StrategyConfig) -> Self {
        match config {
            StrategyConfig::PerpGrid {
                symbol,
                leverage,
                is_isolated,
                upper_price,
                lower_price,
                grid_type,
                grid_count,
                total_investment,
                grid_bias,
                trigger_price, // Added Config Field
            } => Self {
                symbol,
                leverage,
                is_isolated,
                upper_price,
                lower_price,
                grid_type,
                grid_count,
                total_investment,
                grid_bias,
                trigger_price, // Added Config Field
                zones: Vec::new(),
                active_orders: HashMap::new(),
                trade_count: 0,
                state: StrategyState::Initializing,
                start_price: None,
                realized_pnl: 0.0,
                total_fees: 0.0,
                unrealized_pnl: 0.0,
                position_size: 0.0,
                avg_entry_price: 0.0,
            },
            _ => panic!("Invalid config type for PerpGridStrategy"),
        }
    }

    fn initialize_zones(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        if self.grid_count < 2 {
            warn!("[PERP_GRID] Grid count must be at least 2");
            return Err(anyhow!("Grid count must be at least 2"));
        }

        // 1. Get initial data (scoped)
        let last_price = {
            let market_info = match ctx.market_info(&self.symbol) {
                Some(i) => i,
                None => {
                    error!("[PERP_GRID] No market info for {}", self.symbol);
                    return Err(anyhow!("No market info for {}", self.symbol));
                }
            };
            market_info.last_price
        };

        // Generate Levels
        let prices: Vec<f64> = {
            let market_info = ctx.market_info(&self.symbol).unwrap();
            common::calculate_grid_prices(
                self.grid_type.clone(),
                self.lower_price,
                self.upper_price,
                self.grid_count,
            )
            .into_iter()
            .map(|p| market_info.round_price(p))
            .collect()
        };

        let num_zones = self.grid_count as usize - 1;
        let investment_per_zone = self.total_investment / num_zones as f64;

        if investment_per_zone < MIN_NOTIONAL_VALUE {
            let msg = format!(
                "Investment per zone ({:.2}) is less than minimum order value ({}). Increase total_investment or decrease grid_count.",
                investment_per_zone, MIN_NOTIONAL_VALUE
            );
            error!("[PERP_GRID] {}", msg);
            return Err(anyhow!(msg));
        }

        let initial_price = self.trigger_price.unwrap_or(last_price);

        // Validation: Check if wallet has enough margin
        let wallet_balance = ctx.get_perp_available("USDC");
        let max_notional = wallet_balance * self.leverage as f64;

        if max_notional < self.total_investment {
            let msg = format!(
                "Insufficient Margin! Balance: {:.2}, Lev: {}, Max Notional: {:.2}, Required: {:.2}. Bailing out.",
                wallet_balance, self.leverage, max_notional, self.total_investment
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
                let market_info = ctx.market_info(&self.symbol).unwrap();
                let raw_size = investment_per_zone / mid_price;
                market_info.clamp_to_min_notional(raw_size, mid_price, MIN_NOTIONAL_VALUE)
            };

            let (state, is_short_oriented) = match self.grid_bias {
                GridBias::Long => {
                    if initial_price < upper {
                        (ZoneState::WaitingSell, false)
                    } else {
                        (ZoneState::WaitingBuy, false)
                    }
                }
                GridBias::Short => {
                    if initial_price > lower {
                        (ZoneState::WaitingBuy, true)
                    } else {
                        (ZoneState::WaitingSell, true)
                    }
                }
                GridBias::Neutral => {
                    if mid_price > initial_price {
                        (ZoneState::WaitingSell, true)
                    } else {
                        (ZoneState::WaitingBuy, false)
                    }
                }
            };

            if !is_short_oriented && state == ZoneState::WaitingSell {
                total_position_required += size;
            }
            if is_short_oriented && state == ZoneState::WaitingBuy {
                total_position_required -= size; // Short position needed (negative size)
            }

            self.zones.push(GridZone {
                index: i,
                lower_price: lower,
                upper_price: upper,
                size,
                state,
                is_short_oriented,
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

            if let Some(trigger) = self.trigger_price {
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
                let market_info = ctx.market_info(&self.symbol).unwrap();
                let market_price = market_info.last_price;
                let side = if total_position_required > 0.0 {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                };

                let raw_price = if side.is_buy() {
                    // Long Bias: Find highest grid level BELOW market
                    self.zones
                        .iter()
                        .map(|z| z.lower_price)
                        .filter(|&p| p < market_price)
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(market_price)
                } else {
                    // Short Bias: Find lowest grid level ABOVE market
                    self.zones
                        .iter()
                        .map(|z| z.upper_price)
                        .filter(|&p| p > market_price)
                        .min_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(market_price)
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
                    side,
                    target_size,
                    self.symbol,
                    activation_price
                );
                ctx.place_order(OrderRequest::Limit {
                    symbol: self.symbol.clone(),
                    side,
                    price: activation_price,
                    sz: target_size,
                    reduce_only: false,
                    cloid: Some(cloid),
                });
                return Ok(());
            }
        }

        self.state = StrategyState::Running;
        if let Err(e) = self.refresh_orders(ctx) {
            warn!("[PERP_GRID] Failed to refresh orders: {}", e);
        }
        Ok(())
    }

    fn refresh_orders(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        let market_info = match ctx.market_info(&self.symbol) {
            Some(i) => i.clone(),
            None => {
                error!("[PERP_GRID] No market info for {}", self.symbol);
                return Err(anyhow!("No market info for {}", self.symbol));
            }
        };

        for idx in 0..self.zones.len() {
            if self.zones[idx].order_id.is_none() {
                let (side, price, size, reduce_only) = {
                    let zone = &self.zones[idx];
                    let side = if matches!(zone.state, ZoneState::WaitingBuy) {
                        OrderSide::Buy
                    } else {
                        OrderSide::Sell
                    };
                    let price = if side.is_buy() {
                        zone.lower_price
                    } else {
                        zone.upper_price
                    };
                    let reduce_only = if zone.is_short_oriented {
                        side.is_buy()
                    } else {
                        side.is_sell()
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
                    self.symbol,
                    price,
                    if reduce_only { " (RO)" } else { "" }
                );

                ctx.place_order(OrderRequest::Limit {
                    symbol: self.symbol.clone(),
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

    fn place_counter_order(
        &mut self,
        zone_idx: usize,
        price: f64,
        side: OrderSide,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        let zone = &mut self.zones[zone_idx];
        let next_cloid = ctx.generate_cloid();

        let market_info = match ctx.market_info(&self.symbol) {
            Some(i) => i,
            None => {
                error!("[PERP_GRID] No market info for {}", self.symbol);
                return Err(anyhow!("No market info for {}", self.symbol));
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
        // Long Bias: Sell = Close (Reduce), Buy = Open.
        // Short Bias: Buy = Close (Reduce), Sell = Open.
        let reduce_only = if zone.is_short_oriented {
            // Short Bias
            side.is_buy() // Buying to close short
        } else {
            // Long Bias
            side.is_sell() // Selling to close long
        };

        info!(
            "[ORDER_REQUEST] [PERP_GRID] COUNTER_ORDER: LIMIT {} {} {} @ {}{}",
            side,
            rounded_size,
            self.symbol,
            rounded_price,
            if reduce_only { " (RO)" } else { "" }
        );

        self.active_orders.insert(next_cloid, zone_idx);
        zone.order_id = Some(next_cloid);

        ctx.place_order(OrderRequest::Limit {
            symbol: self.symbol.clone(),
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
                if let Some(market_info) = ctx.market_info(&self.symbol) {
                    if market_info.last_price > 0.0 {
                        self.initialize_zones(ctx)?;
                    }
                }
            }
            StrategyState::WaitingForTrigger => {
                if let Some(trigger) = self.trigger_price {
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

                    // Update Position Size
                    if fill.side.is_buy() {
                        self.position_size += fill.size;
                    } else {
                        self.position_size -= fill.size;
                    }

                    // Update Zones Entry Price
                    for zone in &mut self.zones {
                        // Long Bias: We bought, now WaitingSell (Close Long). Set entry.
                        if !zone.is_short_oriented && zone.state == ZoneState::WaitingSell {
                            zone.entry_price = fill.price;
                        }
                        // Short Bias: We sold, now WaitingBuy (Close Short). Set entry.
                        if zone.is_short_oriented && zone.state == ZoneState::WaitingBuy {
                            zone.entry_price = fill.price;
                        }
                    }

                    self.state = StrategyState::Running;
                    self.refresh_orders(ctx)?;
                    return Ok(());
                }
            }

            if let Some(zone_idx) = self.active_orders.remove(&cloid_val) {
                self.trade_count += 1;

                let (next_px, next_side) = {
                    let zone = &mut self.zones[zone_idx];
                    zone.order_id = None;

                    // ============================================================
                    // FILL VALIDATION ASSERTIONS
                    // Verify exchange fill data matches our internal expectations
                    // ============================================================

                    // 1. Validate fill.side matches zone state expectation
                    let expected_side = match zone.state {
                        ZoneState::WaitingBuy => OrderSide::Buy,
                        ZoneState::WaitingSell => OrderSide::Sell,
                    };
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
                    // Long bias: WaitingBuy = "Open Long", WaitingSell = "Close Long"
                    // Short bias: WaitingSell = "Open Short", WaitingBuy = "Close Short"
                    let expected_dir = match (zone.state, zone.is_short_oriented) {
                        (ZoneState::WaitingBuy, false) => "Open Long",
                        (ZoneState::WaitingSell, false) => "Close Long",
                        (ZoneState::WaitingSell, true) => "Open Short",
                        (ZoneState::WaitingBuy, true) => "Close Short",
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
                    let expected_reduce_only = match (zone.state, zone.is_short_oriented) {
                        (ZoneState::WaitingBuy, false) => false,  // Open Long
                        (ZoneState::WaitingSell, false) => true,  // Close Long
                        (ZoneState::WaitingSell, true) => false,  // Open Short
                        (ZoneState::WaitingBuy, true) => true,    // Close Short
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

                    // ============================================================
                    // END FILL VALIDATION
                    // ============================================================

                    // Update Position Size based on Zone State
                    match zone.state {
                        ZoneState::WaitingBuy => {
                            // Buying
                            self.position_size += fill.size;
                        }
                        ZoneState::WaitingSell => {
                            // Selling
                            self.position_size -= fill.size;
                        }
                    }

                    let (next_state, entry_px, _pnl, next_px, next_side) = match (
                        zone.state,
                        zone.is_short_oriented,
                    ) {
                        (ZoneState::WaitingBuy, false) => {
                            info!(
                                "[PERP_GRID] Zone {} | BUY (Open Long) Filled @ {} | Size: {} | Next: SELL (Close) @ {}",
                                zone_idx, fill.price, fill.size, zone.upper_price
                            );
                            (ZoneState::WaitingSell, fill.price, None, zone.upper_price, OrderSide::Sell)
                        }
                        (ZoneState::WaitingSell, false) => {
                            let pnl = (fill.price - zone.entry_price) * fill.size;
                            zone.roundtrip_count += 1;
                            info!(
                                "[PERP_GRID] Zone {} | SELL (Close Long) Filled @ {} | PnL: {:.4} | Next: BUY (Open) @ {}",
                                zone_idx, fill.price, pnl, zone.lower_price
                            );
                            (
                                ZoneState::WaitingBuy,
                                0.0,
                                Some(pnl),
                                zone.lower_price,
                                OrderSide::Buy,
                            )
                        }
                        (ZoneState::WaitingSell, true) => {
                            info!(
                                "[PERP_GRID] Zone {} | SELL (Open Short) Filled @ {} | Size: {} | Next: BUY (Close) @ {}",
                                zone_idx, fill.price, fill.size, zone.lower_price
                            );
                            (ZoneState::WaitingBuy, fill.price, None, zone.lower_price, OrderSide::Buy)
                        }
                        (ZoneState::WaitingBuy, true) => {
                            let pnl = (zone.entry_price - fill.price) * fill.size;
                            zone.roundtrip_count += 1;
                            info!(
                                "[PERP_GRID] Zone {} | BUY (Close Short) Filled @ {} | PnL: {:.4} | Next: SELL (Open) @ {}",
                                zone_idx, fill.price, pnl, zone.upper_price
                            );
                            (
                                ZoneState::WaitingSell,
                                0.0,
                                Some(pnl),
                                zone.upper_price,
                                OrderSide::Sell,
                            )
                        }
                    };

                    zone.state = next_state;
                    zone.entry_price = entry_px;
                    (next_px, next_side)
                };

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

        // Calculate actual total roundtrips from zones
        let total_roundtrips: u32 = self.zones.iter().map(|z| z.roundtrip_count).sum();

        StatusSummary {
            strategy_name: "PerpGrid".to_string(),
            symbol: self.symbol.clone(),
            realized_pnl: self.realized_pnl,
            unrealized_pnl: self.unrealized_pnl,
            total_fees: self.total_fees,
            inventory: InventoryStats {
                base_size: self.position_size,
                avg_entry_price: self.avg_entry_price,
            },
            wallet: WalletStats {
                base_balance: 0.0,
                quote_balance: ctx.get_perp_available("USDC"),
            },
            price: current_mid,
            zones: refined_zones,
            custom: serde_json::json!({
                "leverage": self.leverage,
                "grid_bias": format!("{:?}", self.grid_bias),
                "long_inventory": if self.position_size > 0.0 { self.position_size } else { 0.0 },
                "short_inventory": if self.position_size < 0.0 { self.position_size.abs() } else { 0.0 },
                "state": format!("{:?}", self.state),
                "roundtrips": total_roundtrips,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::context::{MarketInfo, StrategyContext};
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

        let config = StrategyConfig::PerpGrid {
            symbol: symbol.clone(),
            leverage: 10,
            is_isolated: true,
            upper_price: 110.0,
            lower_price: 90.0,
            grid_type: GridType::Arithmetic,
            grid_count: 3, // 90, 100, 110
            total_investment: 1000.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);
        strategy.on_tick(100.0, &mut ctx).unwrap();

        assert_eq!(strategy.zones.len(), 2);
        // Zone 0: 90-100. Price 100 -> WaitingBuy (Low)
        assert_eq!(strategy.zones[0].state, ZoneState::WaitingBuy);
        // Zone 1: 100-110. Price 100 < 110. -> WaitingSell (High)
        assert_eq!(strategy.zones[1].state, ZoneState::WaitingSell);

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

        let config = StrategyConfig::PerpGrid {
            symbol: symbol.clone(),
            leverage: 1,
            is_isolated: true,
            upper_price: 120.0,
            lower_price: 80.0,
            grid_type: GridType::Arithmetic,
            grid_count: 3, // 80, 100, 120
            total_investment: 100.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);
        strategy.on_tick(100.0, &mut ctx).unwrap();

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

        let config = StrategyConfig::PerpGrid {
            symbol: symbol.clone(),
            leverage: 1,
            is_isolated: true,
            upper_price: 120.0,
            lower_price: 80.0,
            grid_type: GridType::Arithmetic,
            grid_count: 3, // 80, 100, 120
            total_investment: 100.0,
            grid_bias: GridBias::Long,
            trigger_price: None,
        };

        let mut strategy = PerpGridStrategy::new(config);
        strategy.on_tick(100.0, &mut ctx).unwrap();

        // 1. Initial State: Empty position
        assert_eq!(strategy.position_size, 0.0);

        // 2. Acquisition Fill (if any)
        // With current setup (Long Bias, Price 100, Range 80-120), zones are:
        // 80-100 (WaitingBuy), 100-120 (WaitingSell).
        // Bias Long + WaitingSell -> Buy to Open.
        // So initialize_zones determines we need position for the upper zone.
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
        assert_eq!(zone0.state, ZoneState::WaitingBuy);
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
        assert_eq!(zone0.state, ZoneState::WaitingSell);
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
}
