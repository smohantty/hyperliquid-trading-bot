# Requirements: Hyperliquid Rust Bot (Phase 2: Core Trading Engine)

## 1. Objective
Build the **Core Trading Engine** that bridges the configuration system (Phase 1) with the Hyperliquid Exchange. This involves integrating the `hyperliquid-rust-sdk`, implementing an event-driven `Engine` to orchestrate data/orders, and defining a real `Strategy` trait for trading logic.

## 2. Technical Stack
-   **SDK**: `hyperliquid-rust-sdk` (Latest).
-   **Async Runtime**: `tokio` (for WebSocket event loop).
-   **Channels**: `tokio::sync::mpsc` (for internal communication if needed).

## 3. Architecture

### 3.1. The "Smart" Engine (`src/engine/`)
The `Engine` will act as a smart orchestrator, abstracting complexity from the strategy.
-   **Responsibilities**:
    1.  **Order Management**: Track all active orders and their fill states.
    2.  **Fill Aggregation**: Listen to fill events. If an order is partially filled, the Engine updates its internal state but **does not** notify the strategy yet.
    3.  **Full Fill Notification**: only when an order is 100% filled, call `strategy.on_order_filled()`.
    4.  **Context**: Provide a `StrategyContext` (or `ctx`) to the strategy for actions (place/cancel), avoiding the need for the strategy to return lists of orders.

### 3.2. Strategy Interface (`src/strategy/mod.rs`)
The strategy becomes purely reactive and simple.

```rust
pub trait Strategy {
    // Called on every price update (throttled/debounced if needed)
    fn on_tick(&mut self, price: f64, ctx: &mut StrategyContext);
    
    // Called ONLY when an order is completely filled
    fn on_order_filled(&mut self, order: &Order, ctx: &mut StrategyContext);
}
```

### 3.3. StrategyContext
A helper struct passed to the strategy to interact with the Engine.
-   **Actions**:
    -   `ctx.place_limit_order(symbol, side, price, size)`
    -   `ctx.cancel_order(order_id)`
-   **Data Access**:
    -   `ctx.market_info(symbol) -> Option<&MarketInfo>`: Retrieve market data/metadata for an asset.

### 3.4. MarketInfo
A unified struct containing both static metadata and dynamic state for an asset.
-   **State**:
    -   `last_price`: The most recent trade price (cached).
    -   `sz_decimals`: Size precision (from exchange metadata).
    -   `price_decimals`: Price precision.
-   **Helpers**:
    -   `round_price(price)`: Returns price rounded to correct decimals.
    -   `round_size(size)`: Returns size rounded to correct decimals.


### 3.3. Integration
-   **`src/main.rs`**:
    -   Load Config & Env.
    -   Initialize `Engine` with Config.
    -   `engine.run().await` (replaces direct `strategy.run()`).

## 4. Implementation Steps
1.  **Dependencies**: Add `hyperliquid-rust-sdk` and `tokio`.
2.  **Wrappers**: Create `src/engine/mod.rs` and `src/engine/client.rs` to wrap SDK complexity.
3.  **Trait Update**: Refactor `Strategy` trait in `src/strategy/mod.rs`.
4.  **Engine Logic**: Implement the WebSocket loop and event dispatching.
5.  **Strategy Update**: Update `SpotGrid` and `PerpGrid` to implement the new trait methods (log logic first).
