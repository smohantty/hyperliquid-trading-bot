# Requirements: Hyperliquid Rust Bot (Phase 3: Feature Improvements & Bug Fixes)

## 1. Full Fill Aggregation
- **Description**: Currently, the `Engine` passes every partial fill to the strategy immediately. This complicates strategy logic (e.g., management of grid levels).
- **Goal**: Modify the `Engine` to track partial fills for each `cloid`. Only call `strategy.on_order_filled` when the entire order size has been filled.
- **Implementation**:
    - Store `cloid` -> `target_size` mapping when orders are placed.
    - Accumulate `filled_size` in a local `Engine` state.
    - Trigger strategy event only upon completion.

## 2. Order Cancellation Support
- **Description**: Strategies currently cannot cancel active orders.
- **Goal**: Add `cancel_order(cloid)` to `StrategyContext` and implement handling in the `Engine`.
- **Implementation**:
    - Add `CancelOrder { cloid: Uuid }` to `OrderRequest` enum.
    - Update `Engine` loop to process cancellation requests via `ExchangeClient`.

## 3. Position Error Handling
- **Description**: If an order is declined or fails, the strategy state might become out of sync.
- **Goal**: Add an `on_order_failed(cloid)` method to the `Strategy` trait to allow strategies to recover or re-place orders.

## 4. Enhanced Logging
- **Description**: Better visibility into order lifecycle.
- **Goal**: Log cumulative fill progress for large orders.
