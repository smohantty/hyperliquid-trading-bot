//! Central configuration constants for hyperliquid-trading-bot.
//!
//! This module contains all tunable parameters and magic numbers used throughout
//! the trading bot. Modify values here to adjust bot behavior without changing
//! business logic.
// =============================================================================
// STRATEGY CONSTANTS
// =============================================================================

use crate::model::Spread;

// =============================================================================
// STRATEGY CONSTANTS
// =============================================================================

/// Order Retry Limits
pub const MAX_ORDER_RETRIES: u32 = 5;

/// Spread & Buffer Configuration (as percentage multipliers)
/// 0.1% spread for off-grid acquisition
pub const ACQUISITION_SPREAD: Spread = Spread::new(0.1);

/// 0.05% buffer for perp grids
pub const INVESTMENT_BUFFER_PERP: Spread = Spread::new(0.05);

/// 0.1% buffer for spot grids
pub const INVESTMENT_BUFFER_SPOT: Spread = Spread::new(0.1);

/// 0.05% fee buffer for spot
pub const FEE_BUFFER: Spread = Spread::new(0.05);

// =============================================================================
// ENGINE TIMER INTERVALS
// =============================================================================

use std::time::Duration;

/// Interval for refreshing account balances (30 seconds)
pub const BALANCE_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

/// Interval for broadcasting status summary updates (5 seconds)
pub const STATUS_SUMMARY_INTERVAL: Duration = Duration::from_secs(5);

/// Interval for order reconciliation checks (2 minutes)
pub const RECONCILIATION_INTERVAL: Duration = Duration::from_secs(2 * 60);
