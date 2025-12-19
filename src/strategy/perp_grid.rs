use crate::config::strategy::{GridBias, GridType, StrategyConfig};
use crate::engine::context::StrategyContext;
use crate::strategy::Strategy;
use anyhow::{anyhow, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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
    entry_price: f64, // Used for PnL calculation
    order_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize)]
struct PerpGridState {
    zones: Vec<GridZone>,
    active_orders: HashMap<Uuid, usize>,
    trade_count: u32,
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

    // Internal State
    zones: Vec<GridZone>,
    active_orders: HashMap<Uuid, usize>, // cloid -> zone_index
    trade_count: u32,
    initialized: bool,
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
                zones: Vec::new(),
                active_orders: HashMap::new(),
                trade_count: 0,
                initialized: false,
            },
            _ => panic!("Invalid config type for PerpGridStrategy"),
        }
    }

    fn initialize_zones(&mut self, current_price: f64, ctx: &mut StrategyContext) -> Result<()> {
        info!(
            "Initializing {} zones for Perp Grid {} with {:?} bias",
            self.grid_count, self.symbol, self.grid_bias
        );

        let price_range = self.upper_price - self.lower_price;
        let interval = price_range / (self.grid_count as f64);

        for i in 0..self.grid_count {
            let lower = match self.grid_type {
                GridType::Arithmetic => self.lower_price + (i as f64 * interval),
                GridType::Geometric => {
                    let ratio =
                        (self.upper_price / self.lower_price).powf(1.0 / self.grid_count as f64);
                    self.lower_price * ratio.powi(i as i32)
                }
            };
            let upper = match self.grid_type {
                GridType::Arithmetic => lower + interval,
                GridType::Geometric => {
                    let ratio =
                        (self.upper_price / self.lower_price).powf(1.0 / self.grid_count as f64);
                    lower * ratio
                }
            };

            // Calculate size with leverage
            // Size = (Total Investment / Grid Count) * Leverage / Middle Price of Zone
            let mid_price = (lower + upper) / 2.0;
            let zone_investment = self.total_investment / (self.grid_count as f64);
            let size = (zone_investment * self.leverage as f64) / mid_price;

            // Determine initial state based on grid_bias
            let state = match self.grid_bias {
                GridBias::Neutral => {
                    if mid_price > current_price {
                        ZoneState::WaitingSell
                    } else {
                        ZoneState::WaitingBuy
                    }
                }
                GridBias::Long => ZoneState::WaitingBuy,
                GridBias::Short => ZoneState::WaitingSell,
            };

            self.zones.push(GridZone {
                index: i as usize,
                lower_price: lower,
                upper_price: upper,
                size,
                state,
                entry_price: 0.0,
                order_id: None,
            });
        }

        self.initialized = true;
        self.refresh_orders(ctx)
    }

    fn refresh_orders(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        for zone in &mut self.zones {
            if zone.order_id.is_none() {
                let cloid = Uuid::new_v4();
                let is_buy = matches!(zone.state, ZoneState::WaitingBuy);
                let price = if is_buy {
                    zone.lower_price
                } else {
                    zone.upper_price
                };

                // Get market precision
                let market_info = ctx
                    .market_info(&self.symbol)
                    .ok_or_else(|| anyhow!("Market info for {} not found", self.symbol))?;

                let rounded_price = market_info.round_price(price);
                let rounded_size = market_info.round_size(zone.size);

                info!(
                    "Zone {} | Placing {:?} order @ {} (cloid: {})",
                    zone.index, zone.state, rounded_price, cloid
                );

                self.active_orders.insert(cloid, zone.index);
                zone.order_id = Some(cloid);

                ctx.place_limit_order(
                    self.symbol.clone(),
                    is_buy,
                    rounded_price,
                    rounded_size,
                    false, // Perps can have reduce_only but grid usually doesn't unless specialized
                    Some(cloid),
                );
            }
        }
        Ok(())
    }
}

impl Strategy for PerpGridStrategy {
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext) -> Result<()> {
        if !self.initialized {
            return self.initialize_zones(price, ctx);
        }
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

                        // Place counter SELL
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

                        // Place counter BUY
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
                            false,
                            Some(next_cloid),
                        );
                    }
                }
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

    // State management
    fn save_state(&self) -> Result<String> {
        let state = PerpGridState {
            zones: self.zones.clone(),
            active_orders: self.active_orders.clone(),
            trade_count: self.trade_count,
        };
        serde_json::to_string(&state).map_err(|e| anyhow!("Serialization error: {}", e))
    }

    fn load_state(&mut self, state: &str) -> Result<()> {
        let state: PerpGridState =
            serde_json::from_str(state).map_err(|e| anyhow!("Deserialization error: {}", e))?;

        self.zones = state.zones;
        self.active_orders = state.active_orders;
        self.trade_count = state.trade_count;
        self.initialized = true;

        info!(
            "Loaded state for {}: {} zones, {} active orders",
            self.symbol,
            self.zones.len(),
            self.active_orders.len()
        );
        Ok(())
    }
}
