# Requirements

### 7. Performance Metrics
- **Strategy Level**:
    - `realized_pnl`: Accumulated from closed trades.
    - `total_fees`: Accumulated execution fees.
    - `unrealized_pnl`: `(Mark - AvgEntry) * Size` (for Long), `(AvgEntry - Mark) * Size` (for Short).
- **Zone Level**:
    - `roundtrip_count`.

## Engine Refactoring (Planned)

The `Engine::run` method has grown too large and complex. To adhere to modern Rust practices and improve maintainability, we will refactor it by extracting distinct responsibilities into helper methods or a dedicated runtime struct.

### Proposed Structure

1.  **Initialization Phase** (Extract specific setup helpers):
    -   `setup_clients()`: Initialize `InfoClient` and `ExchangeClient`.
    -   `load_market_metadata()`: Fetch and build the `HashMap<String, MarketInfo>`.
    -   `fetch_initial_balances()`: Populate `StrategyContext` with initial Spot and Perp balances.

2.  **Event Loop Breakdown** (Extract loop logic):
    -   `process_tick(price, strategy, ctx, pending_orders)`: Handle `AllMids` updates, strategy ticking, and order queue processing.
    -   `process_order_queue(strategy, ctx, exchange_client, pending_orders)`: Handle placement of orders from `ctx.order_queue`.
    -   `process_cancellations(ctx, exchange_client)`: Handle cancellation requests.
    -   `handle_user_event(event, strategy, ctx, pending_orders, completed_cloids)`: Process WebSocket user events (Fills), deduplication, and strategy notification.

3.  **State Management**:
    -   Consider introducing an inner `EngineRuntime` or `EngineState` struct to hold the mutable state (`ctx`, `pending_orders`, `completed_cloids`) so we don't need to pass 5+ arguments to every helper function.

### Goal
-   Reduce `run` method to a high-level orchestration flow.
-   Isolate protocol-specific logic (e.g., parsing API responses) from control flow.
-   Improve testability of individual components.
