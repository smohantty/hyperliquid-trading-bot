# WebSocket API & Event Formats

The bot exposes a WebSocket server (default port `9000`) that broadcasts real-time updates. Frontend applications should consume these events to render the dashboard.

## Connection
*   **URL**: `ws://<HOST>:<PORT>` (e.g., `ws://localhost:9000`)
*   **Protocol**: JSON-based messages.

## Event Envelope
All messages are wrapped in a standard envelope structure if using raw sockets, or sent as direct JSON objects depending on the broadcast implementation. Currently, the bot sends distinct JSON objects with an `event_type` field.

### Status Summary (`summary`)
Sent periodically (e.g., every tick or second) to provide a snapshot of the strategy state.

**Structure**:
```json
{
  "event_type": "summary",
  "data": {
    "strategy_name": "SpotGrid" | "PerpGrid",
    "symbol": "HYPE/USDC",
    "realized_pnl": 123.45,
    "unrealized_pnl": 50.0,
    "total_fees": 1.2,
    "price": 101.5,
    
    "inventory": {
      "base_size": 10.5,
      "avg_entry_price": 99.0
    },
    
    "wallet": {
      "base_balance": 100.0,
      "quote_balance": 5000.0
    },
    
    "zones": [
      {
        "price": 90.0,
        "side": "Buy",      // "Buy" or "Sell"
        "status": "Open",   // "Open" (Active Order) or "Idle" (Waiting)
        "size": 1.0
      }
    ],

    "custom": { ... } // See Strategy-Specific Data below
  }
}
```

## Strategy-Specific Data (`custom`)

The `custom` field in the `summary` event changes schema based on the running strategy.
See the strategy documentation for the authoritative schema:

*   [**Spot Grid Data**](../strategies/spot_grid.md#websocket-data-custom)
*   [**Perp Grid Data**](../strategies/perp_grid.md#websocket-data-custom)

### Spot Grid (`SpotGrid`)
```json
{
  "grid_count": 50,      // Total number of grid lines
  "range_low": 90.0,     // Lower price bound
  "range_high": 110.0,   // Upper price bound
  "roundtrips": 12       // Number of completed buy-sell cycles
}
```

### Perp Grid (`PerpGrid`)
```json
{
  "leverage": 10,
  "grid_bias": "Long" | "Short" | "Neutral",
  "long_inventory": 1000.0,  // Size of Long position
  "short_inventory": 0.0,    // Size of Short position (positive number)
  "state": "Running"         // "Initializing", "WaitingForTrigger", "AcquiringAssets", "Running"
}
```

## Other Events

### Order Update (`order_update`)
Sent whenever an order status changes (New, Filled, Canceled).

```json
{
  "event_type": "order_update",
  "data": {
    "oid": 123456,
    "cloid": "0x...",
    "side": "Buy",
    "price": 99.0,
    "size": 1.0,
    "status": "filled",
    "fee": 0.05
  }
}
```

### Configuration (`config`)
Sent immediately upon connection. Contains the full strategy configuration.

```json
{
  "event_type": "config",
  "data": {
    // ... mirrors the .toml config file ...
    // See "Parameters" in strategy docs:
    // - SpotGrid: ../strategies/spot_grid.md#parameters
    // - PerpGrid: ../strategies/perp_grid.md#parameters
  }
}
```
