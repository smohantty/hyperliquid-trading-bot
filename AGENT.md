# Hyperliquid Trading Bot Agent Context

## Project Goal
Build a robust, modular, and high-performance trading bot for the Hyperliquid exchange (Spot and Perpetual markets) using Rust. The bot features real-time WebSocket interaction and bidirectional grid strategies.

## System Architecture

### 1. Core Engine (`src/engine/`)
- **`mod.rs`**: The heart of the bot. Orchestrates the WebSocket event loop (AllMids, UserEvents), handles order placement via `ExchangeClient`, and routes events to strategies.
- **`context.rs`**: Provides `StrategyContext`, an abstraction layer for strategies to place orders, query market metadata, and check balances without knowing SDK internals.


### 2. Strategy Layer (`src/strategy/`)
- **`mod.rs`**: Defines the `Strategy` trait with methods for `on_tick` and `on_order_filled`.
- **`spot_grid.rs`**: Advanced spot grid with arithmetic/geometric spacing and CLOID-based fill matching.
- **`perp_grid.rs`**: Bidirectional perpetual grid with:
    - **Grid Bias**: `Long`, `Short`, and `Neutral` modes.
    - **Leverage Support**: Dynamic position sizing based on account leverage.
    - **PnL Handling**: Correct entry/exit math for both long and short positions.

### 3. Configuration Management (`src/config/`)
- **`strategy.rs`**: Strong types for all strategy parameters, including `GridBias`, `GridType`, and `leverage`.
- **`creator.rs`**: Interactive CLI wizard using `dialoguer` to safely generate TOML configs.
- **`exchange.rs`**: credential loading from `.env` using `dotenvy`.

## Key Technical Design Decisions

### CLOID Order Matching
The bot uses `uuid::Uuid` as Client Order IDs (CLOIDs).
- **Match-on-Fill**: When a `UserEvent::Fill` arrives, the `Engine` passes the `cloid` to the strategy.
- **Resilience**: Strategies use an `active_orders: HashMap<Uuid, usize>` to instantly map fills back to specific grid zones, even across restarts.

### Safety Mechanisms
- **Safe Mode**: Toggleable in `main.rs` to simulate trading without sending orders to the exchange.
- **Precision Handling**: The bot fetches `szDecimals` and `pxDecimals` from Hyperliquid metadata to ensure all orders satisfy exchange constraints.

## Current Status (Phases 1-6 Complete)
- [x] Core Config & CLI Wizard
- [x] WebSocket Event Loop & Info/Exchange Client Integration
- [x] Spot Grid Strategy (Live Trading Verified)

- [x] Perpetual Grid Strategy (Bidirectional, Biased, Leveraged)
- [x] Testnet Verification (Live Order Flow Confirmed)

## Development workflow
- **Run Bot**: `cargo run -- --config configs/your_config.toml`
- **Interactive Setup**: `cargo run -- --create`
- **Environment**: Ensure `.env` contains `PRIVATE_KEY` and optionally `NETWORK=testnet`.
