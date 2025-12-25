# Spot Grid Strategy (`SpotGrid`)

A classic mean-reversion strategy that buys low and sells high within a defined price range. It holds base assets (e.g., HYPE, ETH) to sell at higher levels and quote assets (e.g., USDC) to buy at lower levels.

## Parameters

| Parameter | Type | Description |
| :--- | :--- | :--- |
| `symbol` | String | Trading pair (e.g., "HYPE/USDC"). |
| `upper_price` | f64 | The highest price of the grid range. Above this, the bot holds all Quote. |
| `lower_price` | f64 | The lowest price of the grid range. Below this, the bot holds all Base. |
| `grid_count` | u32 | Number of price levels. Creates `grid_count - 1` trading zones. |
| `total_investment` | f64 | Total Quote amount allocated to this strategy. |
| `grid_type` | Enum | `Arithmetic` (equal price difference) or `Geometric` (equal ratio). |
| `trigger_price` | Option<f64> | (Optional) Price to start the bot. If set, bot waits until price crosses this level. |

## Logic & State Machine

The strategy operates as a state machine to ensure correct asset allocation before the grid starts. It handles both "Passive Start" (wait for price) and "Active Acquisition" (rebalance portfolio).

### Initialization Flows

1.  **Immediate Start**: No `trigger_price` set, assets are sufficient.
    *   `Initializing` -> `Running`
2.  **Trigger Wait**: Assets sufficient, `trigger_price` set.
    *   `Initializing` -> `WaitingForTrigger` -> (Price hits Trigger) -> `Running`
3.  **Rebalancing (Active Acquisition)**: Assets are insufficient.
    *   `Initializing` -> `AcquiringAssets` (Place Limit Order) -> (Order Filled) -> `Running`

> [!NOTE]
> **Trigger with Deficit**: If a deficit is detected, the bot **immediately** enters `AcquiringAssets` using the `trigger_price` (or current price) as the limit price. It does **not** passively wait for the trigger if it needs to buy/sell assets first.

### State Diagram

```mermaid
stateDiagram-v2
    [*] --> Initializing
    
    state Initializing {
        [*] --> CheckRequirements
        CheckRequirements --> Deficit : Insufficient Assets
        CheckRequirements --> Sufficient : Assets OK
    }

    %% Path 1: Active Rebalancing
    Deficit --> AcquiringAssets : Place Rebalance Order
    AcquiringAssets --> Running : Order Filled
    
    %% Path 2: Passive Trigger
    Sufficient --> WaitingForTrigger : Trigger Defined
    WaitingForTrigger --> Running : Trigger Price Hit
    
    %% Path 3: Direct Start
    Sufficient --> Running : No Trigger
    
    state Running {
        [*] --> GridLogic
        GridLogic --> RefreshOrders
    }
```

## GridZone Architecture

Unlike some grid implementations that track a global "grid index", this bot uses an **Independent Zone Model**. The price range is sliced into $N-1$ distinct **Zones**. Each zone acts as an independent mini-strategy responsible for buying low and selling high within its specific price slice.

### How a Zone Works
Each zone has a fixed `lower_price` and `upper_price`.
1.  **State A (Waiting to Buy)**: The bot holds Quote asset (USDC). It places a buy order at `lower_price`.
2.  **State B (Waiting to Sell)**: The bot holds Base asset (e.g., HYPE). It places a sell order at `upper_price`.

When an order fills, the zone simply transitions to the other state. It does **not** know or care about what adjacent zones are doing.

### Visualization

```mermaid
graph TD
    subgraph Zone2 [Zone 2: 105 - 110]
        Z2_State[Current: Waiting to BUY]
        Z2_Action[Order: Buy @ 105]
    end

    subgraph Zone1 [Zone 1: 100 - 105]
        Z1_State[Current: Waiting to SELL]
        Z1_Action[Order: Sell @ 105]
    end

    subgraph Zone0 [Zone 0: 95 - 100]
        Z0_State[Current: Waiting to BUY]
        Z0_Action[Order: Buy @ 95]
    end

    Price((Current Price: 102))

    Price -.- Zone2
    Price -.- Zone1
    Price -.- Zone0

    style Z1_State fill:#d4edda,stroke:#28a745,color:#155724
    style Z0_State fill:#fff3cd,stroke:#ffc107,color:#856404
    style Z2_State fill:#fff3cd,stroke:#ffc107,color:#856404
```

In the example above:
*   **Zone 2 (105-110)**: Price is below it, so it's waiting for price to drop to 105 to Buy.
*   **Zone 1 (100-105)**: Price is inside it. We previously bought at 100, so now we are waiting to Sell at 105.
*   **Zone 0 (95-100)**: Price is above it, so it's waiting for price to drop to 95 to Buy.

This isolation ensures that even if one zone gets stuck or fails, others continue trading.

### 1. Pre-Flight Validations
Before placing any orders, the bot performs two critical checks:

1.  **Minimum Notional Validation**: Ensures that `total_investment / (grid_count - 1)` is at least **$11.0**. If the investment per zone is too low, the strategy fails to initialize.
2.  **Total Portfolio Value Check**: Calculates `Total Value = (Available Base * Price) + Available Quote`. 
    *   The **Price** used is the `trigger_price` (if set) or current market price.
    *   If `Total Value < total_investment`, the bot **exits with an error**. You must have enough total funds across both assets to cover the requested grid investment.

### 2. Bidirectional Asset Rebalancing
If the total value is sufficient but the specific asset split is wrong for the current price, the bot performs **Active Rebalancing**:

*   **Buying Base**: If you have too much USDC and not enough BTC for the "Sell" zones, the bot places a **Limit Buy** order (at the `initial_price`) to acquire the deficit.
*   **Selling Base**: If you have enough BTC but not enough USDC for the "Buy" zones, the bot places a **Limit Sell** order (at the `initial_price`) to generate the required quote liquidity.

Once the rebalancing order is filled, the bot seeds its internal `inventory` tracking with the actual wallet balance and starts the grid.

### 3. Execution
The grid consists of multiple `zones`:
*   **WaitingBuy**: Current Price > Zone Upper. Ready to buy if price drops.
*   **WaitingSell**: Current Price < Zone Lower. Holding asset, ready to sell if price rises.

When an order fills, the zone "flips" its state and places a counter-order on the opposite boundary.

## Boundary Behavior
*   **Price > Upper Price**: All assets are sold (converted to Quote). Bot waits for price to drop.
*   **Price < Lower Price**: All capital is in Base Asset. Bot waits for price to rise. No new buy orders are placed.

## WebSocket Data (`custom`)
The `status` event contains strategy-specific data in the `custom` field, including `inventory`, `avg_entry_price`, and `total_fees`.
For the authoritative JSON Schema definition, see **[schema.json](../api/schema.json)**.
