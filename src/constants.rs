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
// HELPER FUNCTIONS
// =============================================================================

// Helper functions `markup` and `markdown` are now methods on `Spread`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_markup() {
        let value = 100.0;
        let result = ACQUISITION_SPREAD.markup(value);
        // 100 * (1 + 0.1/100) = 100.1
        assert!((result - 100.1).abs() < 1e-9);
    }
}
