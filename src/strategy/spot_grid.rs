use crate::config::strategy::{GridType, StrategyConfig};
use crate::engine::context::StrategyContext;
use crate::strategy::Strategy;
use anyhow::Result;
use log::{debug, info, warn};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum ZoneState {
    WaitingBuy,
    WaitingSell,
}

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
}

#[allow(dead_code)]
pub struct SpotGridStrategy {
    symbol: String,
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
                // Always start in Initializing to allow balance checks in initialize_zones
                Self {
                    symbol,
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
                }
            }
            _ => panic!("Invalid config type for SpotGridStrategy"),
        }
    }

    fn initialize_zones(&mut self, ctx: &mut StrategyContext) {
        if self.grid_count < 2 {
            warn!("Grid count must be at least 2");
            return;
        }

        let market_info = match ctx.market_info(&self.symbol) {
            Some(info) => info,
            None => return, // Can't init without metadata
        };

        // Generate Levels
        let mut prices = Vec::with_capacity(self.grid_count as usize);
        match self.grid_type {
            GridType::Arithmetic => {
                let step = (self.upper_price - self.lower_price) / (self.grid_count as f64 - 1.0);
                for i in 0..self.grid_count {
                    let mut price = self.lower_price + (i as f64 * step);
                    price = market_info.round_price(price);
                    prices.push(price);
                }
            }
            GridType::Geometric => {
                let ratio = (self.upper_price / self.lower_price)
                    .powf(1.0 / (self.grid_count as f64 - 1.0));
                for i in 0..self.grid_count {
                    let mut price = self.lower_price * ratio.powi(i as i32);
                    price = market_info.round_price(price);
                    prices.push(price);
                }
            }
        }

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
            });
        }

        info!(
            "Setup completed. Total Base required for SELL zones: {}",
            total_base_required
        );

        // Pre-flight check: Assets acquisition
        let base_coin = self.symbol.split('/').next().unwrap_or(&self.symbol);
        let available_base = ctx.balance(base_coin);
        let deficit = total_base_required - available_base;

        // Logic Split:
        // 1. If Deficit > 0: Place Buy Order @ (TriggerPrice OR BestPrice).
        //    If Trigger is set, we buy @ Trigger -> implicitly waiting for trigger.
        // 2. If Deficit <= 0:
        //    If Trigger is set -> WaitingForTrigger (Passive wait).
        //    Else -> Running.

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

            // Apply 0.5% safety buffer to cover exchange fees, ensuring we have enough for SELL orders
            let deficit_with_buffer = deficit * 1.005;
            let min_acq_sz = market_info.ensure_min_sz(acquisition_price, 10.1);
            let rounded_deficit = min_acq_sz.max(market_info.round_size(deficit_with_buffer));

            if rounded_deficit > 0.0 {
                info!(
                    "Acquisition Needed: Deficit of {} {}. Placing BUY order @ {}.",
                    rounded_deficit, base_coin, acquisition_price
                );
                let cloid = ctx.generate_cloid();
                self.state = StrategyState::AcquiringAssets { cloid };

                ctx.place_limit_order(
                    self.symbol.clone(),
                    true,
                    acquisition_price,
                    rounded_deficit,
                    false,
                    Some(cloid),
                );
                return;
            }
        }

        // No Deficit (or negligible)
        if let Some(_trigger) = self.trigger_price {
            // Passive Wait Mode
            info!("Assets sufficient. Entering WaitingForTrigger state.");
            self.start_price = Some(market_info.last_price);
            self.state = StrategyState::WaitingForTrigger;
        } else {
            // No Trigger, Assets OK -> Running
            info!("Assets verified. Starting Grid.");
            self.state = StrategyState::Running;
        }
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

                let market_info = ctx.market_info(&self.symbol).unwrap();
                // Ensure price and size are rounded
                let price = market_info.round_price(price);
                // zone.size is already rounded and min-checked

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
                "Zone {}: Placing {} order at {} (cloid: {})",
                index,
                if is_buy { "BUY" } else { "SELL" },
                price,
                cloid
            );
            ctx.place_limit_order(
                self.symbol.clone(),
                is_buy,
                price,
                size,
                false, // reduce_only
                Some(cloid),
            );
        }
    }
}

impl Strategy for SpotGridStrategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()> {
        match self.state {
            StrategyState::Initializing => {
                if let Some(market_info) = ctx.market_info(&self.symbol) {
                    if market_info.last_price > 0.0 {
                        self.initialize_zones(ctx);
                    }
                }
            }
            StrategyState::WaitingForTrigger => {
                if let Some(trigger) = self.trigger_price {
                    // Directional Trigger Logic
                    // Requires start_price to be set during initialization

                    let mut triggered = false;

                    let start = self
                        .start_price
                        .expect("Start price must be set when in WaitingForTrigger state");

                    if start < trigger {
                        // Bullish Trigger: Wait for price >= trigger
                        if price >= trigger {
                            info!(
                                "Price {} crossed trigger {} (UP). Starting.",
                                price, trigger
                            );
                            triggered = true;
                        }
                    } else {
                        // Bearish Trigger: Wait for price <= trigger
                        // (Or if start == trigger, we trigger immediately/next tick)
                        if price <= trigger {
                            info!(
                                "Price {} crossed trigger {} (DOWN). Starting.",
                                price, trigger
                            );
                            triggered = true;
                        }
                    }

                    if triggered {
                        self.state = StrategyState::Running;
                        self.refresh_orders(ctx);
                    }
                } else {
                    // Should not happen if state is WaitingForTrigger
                    self.state = StrategyState::Running;
                }
            }
            StrategyState::AcquiringAssets { .. } => {
                // Handled in on_order_filled
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
        cloid: Option<u128>,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        if let Some(cloid_val) = cloid {
            // Check for Acquisition Fill
            if let StrategyState::AcquiringAssets { cloid: acq_cloid } = self.state {
                if cloid_val == acq_cloid {
                    info!(
                        "Acquisition order filled! Size: {} @ {}. Starting grid.",
                        size, px
                    );
                    self.state = StrategyState::Running;
                    self.refresh_orders(ctx);
                    return Ok(());
                }
            }

            if let Some(zone_idx) = self.active_orders.remove(&cloid_val) {
                let zone = &mut self.zones[zone_idx];

                if zone.order_id != Some(cloid_val) {
                    warn!(
                        "Zone {} order_id mismatch! Expected {:?}, got {:?}",
                        zone_idx, zone.order_id, cloid_val
                    );
                }
                zone.order_id = None;
                self.trade_count += 1;

                match zone.state {
                    ZoneState::WaitingBuy => {
                        info!(
                            "Zone {} | BUY Filled @ {} | Size: {} | Next: SELL @ {}",
                            zone_idx, px, size, zone.upper_price
                        );

                        zone.state = ZoneState::WaitingSell;
                        zone.entry_price = px;

                        let next_cloid = ctx.generate_cloid();
                        let market_info = ctx.market_info(&self.symbol).unwrap();
                        let price = market_info.round_price(zone.upper_price);

                        info!(
                            "Zone {} | Placing SELL Order @ {} (cloid: {})",
                            zone_idx, price, next_cloid
                        );

                        self.active_orders.insert(next_cloid, zone_idx);
                        zone.order_id = Some(next_cloid);

                        ctx.place_limit_order(
                            self.symbol.clone(),
                            false,
                            price,
                            zone.size,
                            false,
                            Some(next_cloid),
                        );
                    }
                    ZoneState::WaitingSell => {
                        let pnl = (px - zone.entry_price) * size;
                        info!(
                            "Zone {} | SELL Filled @ {} | Size: {} | PnL: {:.4} | Next: BUY @ {}",
                            zone_idx, px, size, pnl, zone.lower_price
                        );

                        zone.state = ZoneState::WaitingBuy;
                        zone.entry_price = 0.0;

                        let next_cloid = ctx.generate_cloid();
                        let market_info = ctx.market_info(&self.symbol).unwrap();
                        let price = market_info.round_price(zone.lower_price);

                        info!(
                            "Zone {} | Placing BUY Order @ {} (cloid: {})",
                            zone_idx, price, next_cloid
                        );

                        self.active_orders.insert(next_cloid, zone_idx);
                        zone.order_id = Some(next_cloid);

                        ctx.place_limit_order(
                            self.symbol.clone(),
                            true,
                            price,
                            zone.size,
                            false,
                            Some(next_cloid),
                        );
                    }
                }
            } else {
                debug!("Fill received for unknown/inactive CLOID: {}", cloid_val);
            }
        } else {
            debug!("Fill received without CLOID at price {}", px);
        }

        Ok(())
    }

    fn on_order_failed(&mut self, cloid: u128, _ctx: &mut StrategyContext) -> Result<()> {
        if let Some(zone_idx) = self.active_orders.remove(&cloid) {
            warn!(
                "Order failed for zone {}. Resetting state to allow retry.",
                zone_idx
            );
            let zone = &mut self.zones[zone_idx];
            zone.order_id = None;
        }
        Ok(())
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
        ctx.set_balance("HYPE".to_string(), 100.0); // Sufficient assets

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
        ctx.set_balance("HYPE".to_string(), 0.0); // Zero assets

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
    }
}
