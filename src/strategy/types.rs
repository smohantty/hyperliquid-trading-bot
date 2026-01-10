use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GridType {
    Arithmetic,
    Geometric,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GridBias {
    Long,
    Short,
}

/// The operational mode of a grid zone - determines position direction
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ZoneMode {
    /// Zone holds long positions: Buy to open, Sell to close (reduce_only)
    /// Profits when price rises (buy low, sell high)
    Long,
    /// Zone holds short positions: Sell to open, Buy to close (reduce_only)
    /// Profits when price falls (sell high, buy low)
    Short,
}

/// Represents a percentage spread for markup/markdown calculations.
///
/// 0.1 means 0.1% (pips).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spread {
    pub value: f64,
}

impl Spread {
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    /// Returns value * (1 + spread/100)
    pub fn markup(&self, value: f64) -> f64 {
        value * (1.0 + (self.value / 100.0))
    }

    /// Returns value * (1 - spread/100)
    pub fn markdown(&self, value: f64) -> f64 {
        value * (1.0 - (self.value / 100.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spread_markup_markdown() {
        let spread = Spread::new(0.1); // 0.1%

        let val = 100.0;
        let up = spread.markup(val);
        // 100 * (1 + 0.001) = 100.1
        assert!((up - 100.1).abs() < 1e-10);

        let down = spread.markdown(val);
        // 100 * (1 - 0.001) = 99.9
        assert!((down - 99.9).abs() < 1e-10);
    }
}
