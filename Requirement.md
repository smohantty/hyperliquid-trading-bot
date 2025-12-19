# Requirements

## Spot Grid Strategy - Performance Tracking

- **Goal**: Track performance metrics at both the Zone level and Strategy level.
- **Metrics**:
    - **PnL**: Realized Profit and Loss from completed grid cycles (Buy -> Sell).
    - **Matched Trades**: A count of completed roundtrips (Buy -> Sell cycles).
    - **Fees**: Total fees paid for trades.
- **Implementation**:
    - Metrics should be stored in the `GridZone` struct.
    - Total strategy performance should be an aggregation of zone metrics.
    - `Accumulated Fees` should be properly handled for partial fills in the Engine.
    - `on_order_filled` should facilitate the update of these metrics.
