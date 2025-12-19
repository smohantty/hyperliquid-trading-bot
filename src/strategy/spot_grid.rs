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
            } => Self {
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
            },
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

        let initial_price = market_info.last_price;

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

        if deficit > market_info.round_size(0.0) {
            // Find nearest grid level price below current price for acquisition buy
            let mut acquisition_price = market_info.last_price * 0.99; // Fallback

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

        info!("Assets verified. Starting Grid.");
        self.state = StrategyState::Running;
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
                    if (price >= trigger
                        && self
                            .zones
                            .first()
                            .map(|z| trigger > z.lower_price)
                            .unwrap_or(false))
                        || (price <= trigger
                            && self
                                .zones
                                .first()
                                .map(|z| trigger < z.lower_price)
                                .unwrap_or(false))
                    {
                        info!(
                            "Trigger price {} hit. Starting initialization/acquisition.",
                            trigger
                        );
                        self.state = StrategyState::Initializing;
                    }
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

    fn save_state(&self) -> Result<String> {
        #[derive(Serialize)]
        struct FullGridState {
            zones: Vec<GridZone>,
            active_orders: HashMap<u128, usize>,
            trade_count: u32,
            state: StrategyState,
        }
        let full_state = FullGridState {
            zones: self.zones.clone(),
            active_orders: self.active_orders.clone(),
            trade_count: self.trade_count,
            state: self.state,
        };
        serde_json::to_string(&full_state)
            .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))
    }

    fn load_state(&mut self, state: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct FullGridState {
            zones: Vec<GridZone>,
            active_orders: HashMap<u128, usize>,
            trade_count: u32,
            state: StrategyState,
        }
        let full_state: FullGridState = serde_json::from_str(state)
            .map_err(|e| anyhow::anyhow!("Deserialization error: {}", e))?;

        self.zones = full_state.zones;
        self.active_orders = full_state.active_orders;
        self.trade_count = full_state.trade_count;
        self.state = full_state.state;

        info!(
            "Loaded state for {}: {} zones, {} active orders, state: {:?}",
            self.symbol,
            self.zones.len(),
            self.active_orders.len(),
            self.state
        );
        Ok(())
    }
}
