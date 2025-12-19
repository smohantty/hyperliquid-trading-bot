# Requirements

## Perpetual Grid Strategy

The Perpetual Grid Strategy adapts the grid trading logic for futures markets, utilizing margin (USDC) and allowing for directional bias (Long or Short).

### 1. Configuration & Data Structures
- **Config Parameters**:
    - `symbol`: e.g., "HYPE" (Perpetual symbols are typically just the asset name).
    - `range`: `upper_price`, `lower_price`.
    - `grid_count`: Number of levels.
    - `grid_type`: Arithmetic.
    - `total_margin`: Amount of USDC allocated.
    - `leverage`: Target leverage (used to calculate position sizes).
    - `bias`: `Long` or `Short` (New Enum).
    - `trigger_price`: Optional start trigger.

- **State Tracking**:
    - `position_size`: Current contracts held (+ for Long, - for Short).
    - `entry_price`: Average entry price of the position.
    - `margin_used`: Estimated margin usage.
    - `realized_pnl`: Closed profit.
    - `unrealized_pnl`: Floating profit based on mark price.

### 2. Initialization & Zone Setup
- **Levels**: Generate levels similar to Spot.
- **Bias Logic**:
    - **Long Bias**: Designed to profit from upward volatility.
        - **Zones Below Price**: `WaitingBuy` (Open Long).
        - **Zones Above Price**: `WaitingSell` (Close Long / Reduce-Only).
    - **Short Bias**: Designed to profit from downward volatility.
        - **Zones Above Price**: `WaitingSell` (Open Short).
        - **Zones Below Price**: `WaitingBuy` (Close Short / Reduce-Only).

### 3. Initial Position Acquisition (Active Start)
- **Goal**: Acquire the necessary inventory to support the "Closing" side of the grid immediately.
- **Long Bias**:
    - Calculate total size of all `WaitingSell` zones (zones > current price).
    - **Action**: Place Market/Limit Long order to acquire this initial position.
    - **Result**: We hold Long position. We place Reduce-Only Sells above. We place Open-Long Buys below.
- **Short Bias**:
    - Calculate total size of all `WaitingBuy` zones (zones < current price).
    - **Action**: Place Market/Limit Short order to acquire this initial position.
    - **Result**: We hold Short position. We place Reduce-Only Buys below. We place Open-Short Sells above.

### 4. Trigger Logic
- Support `trigger_price` (same as Spot).
- If Trigger is set:
    - State `WaitingForTrigger`.
    - Do not acquire position until triggered.
    - Once triggered -> Execute Acquisition -> `Running`.

### 5. Execution Logic (Running)
- **Long Bias**:
    - **Fill Buy (Open Long)**:
        - transition to `WaitingSell`.
        - Place **Reduce-Only Sell** at upper level.
        - Update: `position_size` increases, `avg_entry` updates.
    - **Fill Sell (Close Long)**:
        - transition to `WaitingBuy`.
        - Place **Open Long Buy** at lower level.
        - Update: `position_size` decreases, `realized_pnl` increases.

- **Short Bias**:
    - **Fill Sell (Open Short)**:
        - transition to `WaitingBuy`.
        - Place **Reduce-Only Buy** at lower level.
        - Update: `position_size` decreases (more negative), `avg_entry` updates.
    - **Fill Buy (Close Short)**:
        - transition to `WaitingSell`.
        - Place **Open Short Sell** at upper level.
        - Update: `position_size` increases (less negative), `realized_pnl` increases.

### 6. Order Management
- **Reduce-Only**: Crucial for the "Closing" orders to prevent flipping position accidentally.
- **Margin Check**: Ensure available margin is sufficient before placing Open orders.

### 7. Performance Metrics
- **Strategy Level**:
    - `realized_pnl`: Accumulated from closed trades.
    - `total_fees`: Accumulated execution fees.
    - `unrealized_pnl`: `(Mark - AvgEntry) * Size` (for Long), `(AvgEntry - Mark) * Size` (for Short).
- **Zone Level**:
    - `roundtrip_count`.
