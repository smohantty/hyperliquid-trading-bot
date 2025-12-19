# Requirements

## Spot Grid Strategy - Trigger Price Handling

- **Current Behavior**: The `spot_grid` strategy currently initializes using `last_price` from `market_info`.
- **New Requirement**:
    - If a `trigger_price` is provided, the strategy MUST use it for initialization instead of `last_price`.
    - If asset acquisition is required during initialization, it should be based on the `trigger_price`.
    - After initialization with `trigger_price`, the strategy should proceed with its standard grid logic.
