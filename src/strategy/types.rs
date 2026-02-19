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

impl GridBias {
    pub fn as_str(&self) -> &str {
        match self {
            GridBias::Long => "long",
            GridBias::Short => "short",
        }
    }
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
