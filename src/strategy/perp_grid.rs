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
    is_short_oriented: bool, // Handles Open Short -> Close Short vs Open Long -> Close Long
    entry_price: f64,        // Used for PnL calculation
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
            let (state, is_short_oriented) = match self.grid_bias {
                GridBias::Neutral => {
                    if mid_price > current_price {
                        (ZoneState::WaitingSell, true)
                    } else {
                        (ZoneState::WaitingBuy, false)
                    }
                }
                GridBias::Long => (ZoneState::WaitingBuy, false),
                GridBias::Short => (ZoneState::WaitingSell, true),
            };

            self.zones.push(GridZone {
                index: i as usize,
                lower_price: lower,
                upper_price: upper,
                size,
                state,
                is_short_oriented,
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

    fn place_counter_order(
        &mut self,
        zone_idx: usize,
        price: f64,
        is_buy: bool,
        ctx: &mut StrategyContext,
    ) -> Result<()> {
        let zone = &mut self.zones[zone_idx];
        let next_cloid = Uuid::new_v4();

        let market_info = ctx
            .market_info(&self.symbol)
            .ok_or_else(|| anyhow!("Market info not found"))?;
        let rounded_price = market_info.round_price(price);
        let rounded_size = market_info.round_size(zone.size);

        info!(
            "Zone {} | Placing {:?} order @ {} (cloid: {})",
            zone_idx,
            if is_buy { "BUY" } else { "SELL" },
            rounded_price,
            next_cloid
        );

        self.active_orders.insert(next_cloid, zone_idx);
        zone.order_id = Some(next_cloid);

        ctx.place_limit_order(
            self.symbol.clone(),
            is_buy,
            rounded_price,
            rounded_size,
            false,
            Some(next_cloid),
        );

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
                self.trade_count += 1;

                let (next_px, is_next_buy) = {
                    let zone = &mut self.zones[zone_idx];
                    zone.order_id = None;

                    let (next_state, entry_px, pnl, next_px, is_next_buy) = match (
                        zone.state,
                        zone.is_short_oriented,
                    ) {
                        (ZoneState::WaitingBuy, false) => {
                            // OPEN LONG fill
                            info!(
                                "Zone {} | BUY (Open Long) Filled @ {} | Size: {} | Next: SELL (Close) @ {}",
                                zone_idx, px, size, zone.upper_price
                            );
                            (ZoneState::WaitingSell, px, None, zone.upper_price, false)
                        }
                        (ZoneState::WaitingSell, false) => {
                            // CLOSE LONG fill
                            let pnl = (px - zone.entry_price) * size;
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
                            // OPEN SHORT fill
                            info!(
                                "Zone {} | SELL (Open Short) Filled @ {} | Size: {} | Next: BUY (Close) @ {}",
                                zone_idx, px, size, zone.lower_price
                            );
                            (ZoneState::WaitingBuy, px, None, zone.lower_price, true)
                        }
                        (ZoneState::WaitingBuy, true) => {
                            // CLOSE SHORT fill
                            let pnl = (zone.entry_price - px) * size;
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
                    if let Some(_p) = pnl {
                        // Could track total PnL here
                    }
                    (next_px, is_next_buy)
                };

                // Now we can call self.place_counter_order
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
