use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use uuid::Uuid;

/// Client Order ID - a unique identifier for orders.
/// Wraps a UUID, which the SDK converts to "0x{hex}" on the wire.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cloid(Uuid);

impl Cloid {
    /// Generate a new random cloid (UUID v4)
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the inner UUID (for SDK calls)
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Parse from hex string (with or without 0x prefix)
    /// This is the format returned by fill events from the exchange.
    pub fn from_hex_str(s: &str) -> Option<Self> {
        let normalized = s.strip_prefix("0x").unwrap_or(s);
        u128::from_str_radix(normalized, 16)
            .ok()
            .map(|v| Self(Uuid::from_u128(v)))
    }
}

impl Default for Cloid {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Cloid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cloid({:032x})", self.0.as_u128())
    }
}

impl fmt::Display for Cloid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Wire format: 0x prefix (matches what exchange returns)
        write!(f, "0x{:032x}", self.0.as_u128())
    }
}

// Serialize as hex string (matches Display / wire format)
impl Serialize for Cloid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Cloid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex_str(&s).ok_or_else(|| serde::de::Error::custom("invalid cloid hex string"))
    }
}

/// Order side: Buy or Sell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn is_buy(&self) -> bool {
        matches!(self, OrderSide::Buy)
    }

    pub fn is_sell(&self) -> bool {
        matches!(self, OrderSide::Sell)
    }
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "Buy"),
            OrderSide::Sell => write!(f, "Sell"),
        }
    }
}

/// Represents a filled order notification from the exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFill {
    pub side: OrderSide,
    pub size: f64,
    pub price: f64,
    pub fee: f64,
    pub cloid: Option<Cloid>,
    /// Raw direction from exchange for debugging.
    /// Perps: "Open Long", "Close Long", "Open Short", "Close Short"
    /// Spot: "Buy", "Sell"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderRequest {
    Limit {
        symbol: String,
        side: OrderSide,
        price: f64,
        sz: f64,
        reduce_only: bool,
        cloid: Option<Cloid>,
    },
    Market {
        symbol: String,
        side: OrderSide,
        sz: f64,
        cloid: Option<Cloid>,
    },
    Cancel {
        cloid: Cloid,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloid_display_format() {
        let cloid = Cloid::from_hex_str("0x1234abcd").unwrap();
        let display = cloid.to_string();
        assert!(display.starts_with("0x"));
        assert_eq!(display.len(), 34); // "0x" + 32 hex chars
    }

    #[test]
    fn test_cloid_roundtrip() {
        let original = Cloid::new();
        let hex_str = original.to_string();
        let parsed = Cloid::from_hex_str(&hex_str).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_cloid_from_hex_without_prefix() {
        let cloid = Cloid::from_hex_str("1234abcd").unwrap();
        assert!(cloid.to_string().starts_with("0x"));
    }

    #[test]
    fn test_cloid_serde_roundtrip() {
        let original = Cloid::new();
        let json = serde_json::to_string(&original).unwrap();
        let parsed: Cloid = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }
}
