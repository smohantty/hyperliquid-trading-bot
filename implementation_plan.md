# Implementation Plan: Cloid Type Refactoring

**Linked Requirements**: [requirements.md](./requirements.md)

## Overview

Introduce a `Cloid` newtype in `src/model.rs` that wraps `Uuid` and provides clean conversion methods. Use proper UUID generation (v4 random) instead of u128 counters.

---

## Step 1: Create `Cloid` Type in `src/model.rs`

**File**: `src/model.rs`

```rust
use uuid::Uuid;
use std::fmt;
use serde::{Serialize, Deserialize, Serializer, Deserializer};

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
        // Short format for debug: just the hex without 0x prefix
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
        Self::from_hex_str(&s)
            .ok_or_else(|| serde::de::Error::custom("invalid cloid hex string"))
    }
}
```

---

## Step 2: Update `OrderRequest` in `src/model.rs`

**Change**: `cloid: Option<u128>` → `cloid: Option<Cloid>`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderRequest {
    Limit {
        symbol: String,
        is_buy: bool,
        price: f64,
        sz: f64,
        reduce_only: bool,
        cloid: Option<Cloid>,
    },
    Market {
        symbol: String,
        is_buy: bool,
        sz: f64,
        cloid: Option<Cloid>,
    },
    Cancel {
        cloid: Cloid,
    },
}
```

---

## Step 3: Update `StrategyContext` in `src/engine/context.rs`

**Changes**:
- Import `Cloid` from model
- `cancellation_queue: Vec<Cloid>` (was `Vec<u128>`)
- Remove `next_cloid: u128` field entirely (no longer needed)
- `generate_cloid() -> Cloid` simply returns `Cloid::new()` (UUID v4)
- `cancel_order(cloid: Cloid)`

---

## Step 4: Update Engine (`src/engine/mod.rs`)

**Changes**:
- `pending_orders: HashMap<Cloid, PendingOrder>` (was `HashMap<u128, ...>`)
- `completed_cloids: HashSet<Cloid>` (was `HashSet<u128>`)
- Remove manual hex conversion: use `cloid.to_hex_string()` and `cloid.as_uuid()`
- Fill parsing: use `Cloid::from_hex_str()`
- SDK calls: `cloid.as_uuid()` instead of `Uuid::from_u128(...)`

---

## Step 5: Update Strategies

### `src/strategy/mod.rs`
- Trait method signatures: `cloid: Option<Cloid>` and `on_order_failed(cloid: Cloid, ...)`

### `src/strategy/spot_grid.rs`
- `StrategyState::AcquiringAssets { cloid: Cloid }`
- `active_orders: HashMap<Cloid, usize>`
- Zone `order_id: Option<Cloid>`

### `src/strategy/perp_grid.rs`
- Same changes as spot_grid

---

## Step 6: Verify Unchanged Components

These files use `String` for cloid and should remain unchanged:
- `src/broadcast/types.rs` - `OrderEvent.cloid: Option<String>` (API contract)
- `src/logging/order_audit.rs` - `OrderRecord.cloid: Option<String>` (CSV logging)

Engine will call `.to_hex_string()` when populating these.

---

## Verification Steps

1. **Compile**: `cargo check`
2. **Format**: `cargo fmt`
3. **Test**: `cargo test`
4. **Manual Check**: Ensure cloid round-trips correctly:
   - Generate cloid → send order → receive fill → parse cloid → compare

---

## File Change Summary

| File | Type of Change |
|------|----------------|
| `src/model.rs` | Add `Cloid` type, update `OrderRequest` |
| `src/engine/context.rs` | Update types and methods |
| `src/engine/mod.rs` | Major refactor of cloid handling |
| `src/strategy/mod.rs` | Update trait signatures |
| `src/strategy/spot_grid.rs` | Update types |
| `src/strategy/perp_grid.rs` | Update types |

---

## Rollback Plan

If issues arise:
1. Changes are isolated per file - can revert individually
2. Git revert if needed

---

**⏸️ AWAITING USER APPROVAL BEFORE IMPLEMENTATION**

