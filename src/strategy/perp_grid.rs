use crate::config::strategy::{GridBias, GridType, StrategyConfig};
use crate::engine::context::StrategyContext;
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum ZoneState {
    WaitingBuy,
    WaitingSell,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum StrategyState {
    Initializing,
    WaitingForTrigger,
    AcquiringAssets { cloid: u128, target_size: f64 },
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
    order_id: Option<u128>,

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
    active_orders: HashMap<u128, usize>, // cloid -> zone_index
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

    fn initialize_zones(&mut self, ctx: &mut StrategyContext) {
        if self.grid_count < 2 {
            warn!("Grid count must be at least 2");
            return;
        }

        // 1. Get initial data (scoped)
        let last_price = {
            let info = match ctx.market_info(&self.symbol) {
                Some(i) => i,
                None => {
                    warn!("No market info");
                    return;
                }
            };
            info.last_price
        };

        // Generate Levels (Pure calculation, no borrow)
        let price_range = self.upper_price - self.lower_price;
        let interval = price_range / (self.grid_count as f64 - 1.0);
        let mut prices = Vec::with_capacity(self.grid_count as usize);

        // Re-borrow for rounding if strict, or just use raw math and round later?
        // Better to round prices once.
        {
            let info = ctx.market_info(&self.symbol).unwrap();
            for i in 0..self.grid_count {
                let price = match self.grid_type {
                    GridType::Arithmetic => self.lower_price + (i as f64 * interval),
                    GridType::Geometric => {
                        let ratio = (self.upper_price / self.lower_price)
                            .powf(1.0 / (self.grid_count as f64 - 1.0));
                        self.lower_price * ratio.powi(i as i32)
                    }
                };
                prices.push(info.round_price(price));
            }
        }

        let num_zones = self.grid_count as usize - 1;
        let investment_per_zone = self.total_investment / num_zones as f64;
        let initial_price = self.trigger_price.unwrap_or(last_price);

        // Validation: Check if wallet has enough margin
        let wallet_balance = ctx.balance("USDC");

        let max_notional = wallet_balance * self.leverage as f64;

        if max_notional < self.total_investment {
            warn!(
                "Insufficient Margin! Balance: {}, Lev: {}, Max Notional: {}, Required: {}. Bailing out.",
                wallet_balance, self.leverage, max_notional, self.total_investment
            );
            return;
        }

        self.zones.clear();
        let mut total_position_required = 0.0;

        for i in 0..num_zones {
            let lower = prices[i];
            let upper = prices[i + 1];
            let mid_price = (lower + upper) / 2.0;

            // Borrow for size calc
            let size = {
                let info = ctx.market_info(&self.symbol).unwrap();
                // total_investment is Notional.
                // Size = Notional / Price
                let raw_size = investment_per_zone / mid_price;
                info.ensure_min_sz(mid_price, 10.0)
                    .max(info.round_size(raw_size))
            };

            let (state, is_short_oriented) = match self.grid_bias {
                GridBias::Long => {
                    if initial_price < upper {
                        if initial_price < upper {
                            (ZoneState::WaitingSell, false)
                        } else {
                            (ZoneState::WaitingBuy, false)
                        }
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
                total_position_required -= size;
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
            "Setup completed. Net position required: {}",
            total_position_required
        );

        if total_position_required.abs() > 0.0 {
            self.start_price = Some(initial_price);
            if let Some(trigger) = self.trigger_price {
                info!(
                    "Assets required ({}), but waiting for trigger price {}",
                    total_position_required, trigger
                );
                self.state = StrategyState::WaitingForTrigger;
                return;
            }

            // Acquire Immediately
            info!("Acquiring initial position: {}", total_position_required);
            // Generate CLOID (Mutable Borrow)
            let cloid = ctx.generate_cloid();

            let (activation_price, target_size, is_buy) = {
                // Borrow Info (Immutable)
                let info = ctx.market_info(&self.symbol).unwrap();
                // Use nearest grid level for acquisition (Passive/Grid-Aligned)
                let market_price = info.last_price;
                let is_buy = total_position_required > 0.0;

                let raw_price = if is_buy {
                    // Long Bias/Buy: Find highest grid level BELOW market price
                    self.zones
                        .iter()
                        .map(|z| z.lower_price)
                        .filter(|&p| p < market_price)
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(market_price)
                } else {
                    // Short Bias/Sell: Find lowest grid level ABOVE market price
                    self.zones
                        .iter()
                        .map(|z| z.upper_price)
                        .filter(|&p| p > market_price)
                        .min_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(market_price)
                };
                (
                    info.round_price(raw_price),
                    info.round_size(total_position_required.abs()),
                    is_buy,
                )
            }; // Drop Info

            if target_size > 0.0 {
                self.state = StrategyState::AcquiringAssets { cloid, target_size };
                // Call Place Order (Mutable Borrow)
                ctx.place_limit_order(
                    self.symbol.clone(),
                    is_buy,
                    activation_price,
                    target_size,
                    false,
                    Some(cloid),
                );
                return;
            }
        }

        self.state = StrategyState::Running;
        self.refresh_orders(ctx)
            .unwrap_or_else(|e| warn!("Failed to refresh orders: {}", e));
    }

    fn refresh_orders(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        let mut orders_to_place = Vec::new();

        // 1. Identify needed orders (No ctx borrow needed, relying on self state)
        // BUT we need market info to calculate/round prices? Yes.
        // So we iterate state, collect raw intent.

        let mut intents = Vec::new();
        for zone in &self.zones {
            if zone.order_id.is_none() {
                let is_buy = matches!(zone.state, ZoneState::WaitingBuy);
                let price = if is_buy {
                    zone.lower_price
                } else {
                    zone.upper_price
                };
                intents.push((zone.index, is_buy, price, zone.size, zone.is_short_oriented));
            }
        }

        // 2. Process intents
        for (idx, is_buy, raw_price, raw_size, is_short_linked) in intents {
            let cloid = ctx.generate_cloid();

            let (price, size) = {
                let info = ctx
                    .market_info(&self.symbol)
                    .ok_or_else(|| anyhow!("No market info"))?;
                (info.round_price(raw_price), info.round_size(raw_size))
            };

            self.zones[idx].order_id = Some(cloid);
            self.active_orders.insert(cloid, idx);

            let reduce_only = if is_short_linked { is_buy } else { !is_buy };

            orders_to_place.push((cloid, is_buy, price, size, reduce_only));
        }

        for (cloid, is_buy, price, size, reduce_only) in orders_to_place {
            info!(
                "Placing {:?} order @ {} (cloid: {}, reduce_only: {})",
                if is_buy { "BUY" } else { "SELL" },
                price,
                cloid,
                reduce_only
            );
            ctx.place_limit_order(
                self.symbol.clone(),
                is_buy,
                price,
                size,
                reduce_only,
                Some(cloid),
            );
        }
        Ok(())
    }

    fn place_counter_order(
        &mut self,
        zone_idx: usize,
        price: f64,
        is_buy: bool,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        let zone = &mut self.zones[zone_idx];
        let next_cloid = ctx.generate_cloid();

        let (rounded_price, rounded_size) = {
            let market_info = ctx
                .market_info(&self.symbol)
                .ok_or_else(|| anyhow!("Market info not found"))?;
            (
                market_info.round_price(price),
                market_info.round_size(zone.size),
            )
        };

        info!(
            "Zone {} | Placing {:?} order @ {} (cloid: {})",
            zone_idx,
            if is_buy { "BUY" } else { "SELL" },
            rounded_price,
            next_cloid
        );

        self.active_orders.insert(next_cloid, zone_idx);
        zone.order_id = Some(next_cloid);

        // Determine Reduce-Only for Counter Order
        // Counter order is the "closing" or "next step" order.
        // If we just filled Opening (WaitingBuy, LongBias), next is Closing (WaitingSell).
        // So reduce_only logic should be standard:
        // Long Bias: Sell = Close (Reduce), Buy = Open.
        // Short Bias: Buy = Close (Reduce), Sell = Open.

        let reduce_only = if zone.is_short_oriented {
            // Short Bias
            is_buy // Buying to close short
        } else {
            // Long Bias
            !is_buy // Selling to close long
        };

        ctx.place_limit_order(
            self.symbol.clone(),
            is_buy,
            rounded_price,
            rounded_size,
            reduce_only,
            Some(next_cloid),
        );

        Ok(())
    }
}

impl Strategy for PerpGridStrategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()> {
        match self.state {
            StrategyState::Initializing => {
                self.initialize_zones(ctx);
            }
            StrategyState::WaitingForTrigger => {
                if let Some(trigger) = self.trigger_price {
                    let mut triggered = false;
                    // Need start_price to know direction??
                    // initialize_zones sets self.start_price
                    if let Some(start) = self.start_price {
                        if start < trigger {
                            // Bullish Trigger
                            if price >= trigger {
                                info!(
                                    "Price {} crossed trigger {} (UP). Starting.",
                                    price, trigger
                                );
                                triggered = true;
                            }
                        } else {
                            // Bearish Trigger
                            if price <= trigger {
                                info!(
                                    "Price {} crossed trigger {} (DOWN). Starting.",
                                    price, trigger
                                );
                                triggered = true;
                            }
                        }
                    } else {
                        // Fallback if start_price missing? Should not happen.
                        if price >= trigger {
                            triggered = true;
                        }
                    }

                    if triggered {
                        // Re-run initialization to calculate acquisition needs based on NEW price level?
                        // Or just use the original plan?
                        // Actually, if price moved, the required position calculation might change!
                        // But initialize_zones uses current config.
                        // Let's re-run initialize_zones to be safe and accurate with current price?
                        // NO, initialize_zones regenerates the grid based on config. Config hasn't changed.
                        // But the zones' "state" calculation (WaitingBuy vs WaitingSell) depends on price.
                        // So yes, we should probably re-init zones with the trigger price context.
                        // However, initialize_zones handles allocation.
                        // Let's just re-call verify_assets logic?

                        // Simplest: Just call initialize_zones again. It enters Running or Acquiring.
                        info!("Triggered! Re-initializing zones for accurate state.");
                        self.zones.clear();
                        self.initialize_zones(ctx);
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
                    .unwrap_or_else(|e| warn!("Failed refresh: {}", e));
            }
        }
        Ok(())
    }

    fn on_order_filled(
        &mut self,
        side: &str,
        size: f64,
        px: f64,
        _fee: f64,
        cloid: Option<u128>,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        if let Some(cloid_val) = cloid {
            // Check for Acquisition Fill
            if let StrategyState::AcquiringAssets {
                cloid: acq_cloid, ..
            } = self.state
            {
                if cloid_val == acq_cloid {
                    info!("Acquisition filled @ {}", px);

                    // Update Position Size
                    if side.eq_ignore_ascii_case("buy") {
                        self.position_size += size;
                    } else {
                        self.position_size -= size;
                    }

                    // Update Zones Entry Price
                    for zone in &mut self.zones {
                        // Long Bias: We bought, now WaitingSell (Close Long). Set entry.
                        if !zone.is_short_oriented && zone.state == ZoneState::WaitingSell {
                            zone.entry_price = px;
                        }
                        // Short Bias: We sold, now WaitingBuy (Close Short). Set entry.
                        if zone.is_short_oriented && zone.state == ZoneState::WaitingBuy {
                            zone.entry_price = px;
                        }
                    }

                    self.state = StrategyState::Running;
                    self.refresh_orders(ctx)?;
                    return Ok(());
                }
            }

            if let Some(zone_idx) = self.active_orders.remove(&cloid_val) {
                self.trade_count += 1;

                let (next_px, is_next_buy) = {
                    let zone = &mut self.zones[zone_idx];
                    zone.order_id = None;

                    // Update Position Size based on Zone State
                    match zone.state {
                        ZoneState::WaitingBuy => {
                            // Buying
                            self.position_size += size;
                        }
                        ZoneState::WaitingSell => {
                            // Selling
                            self.position_size -= size;
                        }
                    }

                    let (next_state, entry_px, _pnl, next_px, is_next_buy) = match (
                        zone.state,
                        zone.is_short_oriented,
                    ) {
                        (ZoneState::WaitingBuy, false) => {
                            info!(
                                "Zone {} | BUY (Open Long) Filled @ {} | Size: {} | Next: SELL (Close) @ {}",
                                zone_idx, px, size, zone.upper_price
                            );
                            (ZoneState::WaitingSell, px, None, zone.upper_price, false)
                        }
                        (ZoneState::WaitingSell, false) => {
                            let pnl = (px - zone.entry_price) * size;
                            zone.roundtrip_count += 1;
                            info!(
                                "Zone {} | SELL (Close Long) Filled @ {} | PnL: {:.4} | Next: BUY (Open) @ {}",
                                zone_idx, px, pnl, zone.lower_price
                            );
                            (
                                ZoneState::WaitingBuy,
                                0.0,
                                Some(pnl),
                                zone.lower_price,
                                true,
                            )
                        }
                        (ZoneState::WaitingSell, true) => {
                            info!(
                                "Zone {} | SELL (Open Short) Filled @ {} | Size: {} | Next: BUY (Close) @ {}",
                                zone_idx, px, size, zone.lower_price
                            );
                            (ZoneState::WaitingBuy, px, None, zone.lower_price, true)
                        }
                        (ZoneState::WaitingBuy, true) => {
                            let pnl = (zone.entry_price - px) * size;
                            zone.roundtrip_count += 1;
                            info!(
                                "Zone {} | BUY (Close Short) Filled @ {} | PnL: {:.4} | Next: SELL (Open) @ {}",
                                zone_idx, px, pnl, zone.upper_price
                            );
                            (
                                ZoneState::WaitingSell,
                                0.0,
                                Some(pnl),
                                zone.upper_price,
                                false,
                            )
                        }
                    };

                    zone.state = next_state;
                    zone.entry_price = entry_px;
                    (next_px, is_next_buy)
                };

                self.place_counter_order(zone_idx, next_px, is_next_buy, ctx)?;
            } else {
                debug!(
                    "Fill received for unknown/inactive Perp CLOID: {}",
                    cloid_val
                );
            }
        } else {
            debug!(
                "Fill received without CLOID in PerpStrategy at price {}",
                px
            );
        }
        Ok(())
    }

    fn on_order_failed(&mut self, _cloid: u128, _ctx: &mut StrategyContext) -> Result<()> {
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
                quote_balance: ctx.balance("USDC"),
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
        ctx.set_balance("USDC".to_string(), 10000.0);

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
        ctx.set_balance("USDC".to_string(), 1000.0);

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
            .on_order_filled("buy", 0.5, 100.0, 0.0, Some(cloid), &mut ctx)
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
                crate::model::OrderRequest::Limit { is_buy, .. } => *is_buy,
                crate::model::OrderRequest::Market { is_buy, .. } => *is_buy,
                _ => false,
            })
            .collect();

        let sell_orders: Vec<_> = orders
            .iter()
            .filter(|o| match o {
                crate::model::OrderRequest::Limit { is_buy, .. } => !*is_buy,
                crate::model::OrderRequest::Market { is_buy, .. } => !*is_buy,
                _ => false,
            })
            .collect();

        assert_eq!(buy_orders.len(), 1);
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
        ctx.set_balance("USDC".to_string(), 1000.0);

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
                .on_order_filled("buy", target_size, 100.0, 0.0, Some(cloid), &mut ctx)
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
            .on_order_filled("buy", size, 80.0, 0.0, Some(order_id), &mut ctx)
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
            .on_order_filled("sell", size, 100.0, 0.0, Some(sell_oid), &mut ctx)
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
