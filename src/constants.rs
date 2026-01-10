//! Central configuration constants for hyperliquid-trading-bot.
//!
//! This module contains all tunable parameters and magic numbers used throughout
//! the trading bot. Modify values here to adjust bot behavior without changing
//! business logic.
// =============================================================================
// STRATEGY CONSTANTS
// =============================================================================

/// Order Retry Limits
pub const MAX_ORDER_RETRIES: u32 = 5;

/// Spread & Buffer Configuration (as percentage multipliers)
/// 0.1% spread for off-grid acquisition
pub const ACQUISITION_SPREAD: f64 = 0.001;

/// 0.1% buffer for spot grids
pub const INVESTMENT_BUFFER_SPOT: f64 = 0.001;

/// 0.05% fee buffer for spot
pub const FEE_BUFFER: f64 = 0.0005;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Apply a markup percentage to a value.
/// E.g., with pct=0.001 (0.1%), returns value * 1.001
#[inline]
pub fn markup(value: f64, pct: f64) -> f64 {
    value * (1.0 + pct)
}

/// Apply a markdown percentage to a value.
/// E.g., with pct=0.001 (0.1%), returns value * 0.999
#[inline]
pub fn markdown(value: f64, pct: f64) -> f64 {
    value * (1.0 - pct)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markup() {
        let value = 100.0;
        let result = markup(value, 0.001); // 0.1%
        assert!((result - 100.1).abs() < 1e-9);
    }

    #[test]
    fn test_markdown() {
        let value = 100.0;
        let result = markdown(value, 0.001); // 0.1%
        assert!((result - 99.9).abs() < 1e-9);
    }
}
