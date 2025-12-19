use crate::config::strategy::{GridType, StrategyConfig};
use crate::engine::context::StrategyContext;
use crate::strategy::Strategy;
use anyhow::Result;
use log::{debug, info, warn};
use std::collections::HashMap;
use uuid::Uuid;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum ZoneState {
    WaitingBuy,
    WaitingSell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GridZone {
    index: usize,
    lower_price: f64,
    upper_price: f64,
    size: f64,
    state: ZoneState,
    entry_price: f64,
    order_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize)]
struct SpotGridState {
    zones: Vec<GridZone>,
    active_orders: HashMap<Uuid, usize>,
    trade_count: u32,
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
    active_orders: HashMap<Uuid, usize>,
    initialized: bool,
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
                initialized: false,
                trade_count: 0,
            },
            _ => panic!("Invalid config type for SpotGridStrategy"),
        }
    }

    fn initialize_zones(&mut self, ctx: &StrategyContext) {
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
        for i in 0..num_zones {
            let lower = prices[i];
            let upper = prices[i + 1];

            // Calculate size based on quote investment per zone
            // Use lower price as conservative estimate for size calculation
            let raw_size = quote_per_zone / lower;
            let size = market_info.round_size(raw_size);

            let initial_state = if initial_price < upper {
                ZoneState::WaitingSell
            } else {
                ZoneState::WaitingBuy
            };

            let entry_price = if initial_state == ZoneState::WaitingSell {
                initial_price
            } else {
                0.0
            };

            self.zones.push(GridZone {
                index: i,
                lower_price: lower,
                upper_price: upper,
                size,
                state: initial_state,
                entry_price,
                order_id: None,
            });
        }

        info!("Initialized {} zones for {}", self.zones.len(), self.symbol);
        self.initialized = true;
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
                // zone.size is already rounded

                let cloid = Uuid::new_v4();
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
        // Initialize if needed
        if !self.initialized {
            // We need market info for init
            if ctx.market_info(&self.symbol).is_some() {
                self.initialize_zones(ctx);
            }
        }

        if self.initialized {
            self.refresh_orders(ctx);
        }

        debug!("SpotGridStrategy tick: {}", price);
        Ok(())
    }

    fn on_order_filled(
        &mut self,
        _side: &str,
        size: f64,
        px: f64,
        cloid: Option<uuid::Uuid>,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        if let Some(cloid_val) = cloid {
            if let Some(zone_idx) = self.active_orders.remove(&cloid_val) {
                let zone = &mut self.zones[zone_idx];

                // Verify order match (redundant but safe)
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
                        // FILL was a BUY. Transition to WaitingSell.
                        info!(
                            "Zone {} | BUY Filled @ {} | Size: {} | Next: SELL @ {}",
                            zone_idx, px, size, zone.upper_price
                        );

                        zone.state = ZoneState::WaitingSell;
                        zone.entry_price = px;

                        // Place Counter-Order (Sell)
                        let next_cloid = Uuid::new_v4();
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
                            false, // Sell
                            price,
                            zone.size,
                            false, // reduce_only
                            Some(next_cloid),
                        );
                    }
                    ZoneState::WaitingSell => {
                        // FILL was a SELL. Transition to WaitingBuy.
                        let pnl = (px - zone.entry_price) * size;
                        info!(
                            "Zone {} | SELL Filled @ {} | Size: {} | PnL: {:.4} | Next: BUY @ {}",
                            zone_idx, px, size, pnl, zone.lower_price
                        );

                        zone.state = ZoneState::WaitingBuy;
                        zone.entry_price = 0.0; // Reset cost basis

                        // Place Counter-Order (Buy)
                        let next_cloid = Uuid::new_v4();
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
                            true, // Buy
                            price,
                            zone.size,
                            false, // reduce_only
                            Some(next_cloid),
                        );
                    }
                }
            } else {
                debug!("Fill received for unknown/inactive CLOID: {}", cloid_val);
            }
        } else {
            // No CLOID, can't match to zone easily.
            // In a real bot, we might fallback to price matching or ignore.
            debug!("Fill received without CLOID at price {}", px);
        }

        Ok(())
    }

    // State management
    fn save_state(&self) -> Result<String> {
        let state = SpotGridState {
            zones: self.zones.clone(),
            active_orders: self.active_orders.clone(),
            trade_count: self.trade_count,
        };
        serde_json::to_string(&state).map_err(|e| anyhow::anyhow!("Serialization error: {}", e))
    }

    fn load_state(&mut self, state: &str) -> Result<()> {
        let state: SpotGridState = serde_json::from_str(state)
            .map_err(|e| anyhow::anyhow!("Deserialization error: {}", e))?;

        self.zones = state.zones;
        self.active_orders = state.active_orders;
        self.trade_count = state.trade_count;
        self.initialized = true; // If we loaded state, we are initialized

        info!(
            "Loaded state for {}: {} zones, {} active orders",
            self.symbol,
            self.zones.len(),
            self.active_orders.len()
        );
        Ok(())
    }
}
