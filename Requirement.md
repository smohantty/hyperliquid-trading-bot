
## Phase 4: Smart Strategy Initialization & Balance Management

### 1. Asset Balance Fetching
- **Description**: Strategies currently fly blind regarding account balances.
- **Goal**: The `Engine` should fetch user balances (Spot tokens and USDC margin) and expose them via `StrategyContext`.
- **Implementation**:
    - Update `StrategyContext` to include a `balances: HashMap<String, f64>` or similar.
    - Engine calls `info_client.spot_clearing_info()` and `info_client.clearing_info()` periodically or at startup.

### 2. Spot Grid Pre-Flight Check & Acquisition
- **Description**: Starting a spot grid often requires an initial base asset balance to place sell orders.
- **Goal**: Calculate exactly how much Base and Quote asset is needed for the configured grid. If Base is lacking, automatically acquire it.
- **Step-by-Step Flow**:
    1. **Calculate Needs**: Based on current price, determine which zones are "Sell" zones. Sum their `size` (Base needed). Sum the `size * price` for "Buy" zones (Quote needed).
    2. **Compare with Balances**: Check if current `available_base` and `available_quote` cover the needs.
    3. **Acquire Deficit**: If `available_base < required_base`, place an initial Limit/Market Buy order for the difference.
    4. **Wait for Fill**: Use the "Full Fill Aggregation" logic from Phase 3. Only proceed to full grid placement once the acquisition order is filled.
    5. **Start Grid**: Place all grid orders once assets are verified.

### 3. Initialization State Machine
- **Description**: To support the acquisition flow, strategies need more than a boolean `initialized` flag.
- **Proposed States**: `WaitingForTrigger`, `AcquiringAssets`, `Running`.
