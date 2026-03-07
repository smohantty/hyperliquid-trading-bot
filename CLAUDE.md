# CLAUDE.md - Long-Term Memory for Claude Code

## Memory Metadata
- **Last refreshed:** 2026-02-20
- **Project status:** Active development

## Project Overview
High-performance, event-driven trading bot for the Hyperliquid DEX. Supports Spot and Perpetual Grid Trading strategies. Written in Rust with async I/O (tokio) and the `hyperliquid_rust_sdk`.

**Author:** subhransu (smohantty@gmail.com)

## Architecture

### Components
- **Entry point:** `src/main.rs` -- CLI args (clap), config loading, engine launch (live or simulation)
- **Live Engine:** `src/engine/live.rs` -- WebSocket events, order lifecycle, error recovery, batch submission (977 lines, largest file)
- **Simulation Engine:** `src/engine/simulation.rs` -- Dry-run with simulated fills
- **Common Engine:** `src/engine/common.rs` -- Shared engine utilities
- **Strategy Context:** `src/engine/context.rs` -- Sandbox for strategies: balances, open orders, market info. Strategies NEVER call exchange APIs directly.
- **Perp Grid:** `src/strategy/perp_grid.rs` -- Leveraged grid trading with LONG/SHORT bias, position tracking
- **Spot Grid:** `src/strategy/spot_grid.rs` -- Neutral spot grid trading, inventory tracking
- **Strategy Trait:** `src/strategy/mod.rs` -- `on_tick`, `on_order_filled`, `on_order_failed`, `get_summary`, `get_grid_state`
- **Common Strategy:** `src/strategy/common.rs` -- Grid calculation utilities
- **Strategy Types:** `src/strategy/types.rs` -- GridType, GridBias, ZoneMode, Spread, StrategyState, GridState
- **Models:** `src/model.rs` -- Cloid (UUID-based), OrderSide, OrderFill, OrderRequest (Limit/Market/Cancel)
- **Config:** `src/config/` -- TOML-based: strategy.rs (schemas + validation), exchange.rs, broadcast.rs, simulation.rs, creator.rs (interactive wizard)
- **Broadcast:** `src/broadcast/server.rs` -- WebSocket status server (default port 9000), `types.rs` event types
- **Logging:** `src/logging/order_audit.rs` -- CSV audit trail for orders
- **UI:** `src/ui/console.rs` -- Console grid visualization for dry-run mode
- **Error:** `src/error.rs` -- Error types

### Event Flow
```
Hyperliquid Exchange (WebSocket)
    -> Engine Event Loop (tokio single-thread)
        -> Strategy.on_tick(price)
        -> Strategy.on_order_filled(fill)
        -> Strategy.on_order_failed(cloid)
            -> StrategyContext.order_queue
                -> Engine -> Hyperliquid Exchange
    -> StatusBroadcaster (port 9000)
        -> Dashboard / external daemons / CLI clients
```

### Key Patterns
- Single-threaded async event loop (tokio) prevents race conditions
- Strategy trait pattern for pluggable strategies without runtime overhead
- Context-based safety: strategies never call APIs directly
- All strategy calls wrapped in error handling -- individual failures don't crash bot
- Cloid (Client Order ID) uses UUID with hex serialization for type safety
- Broadcasting caches initial state (config) for instant sync of new clients

## Development Commands
- **Build:** `cargo build --release`
- **Run live:** `cargo run --release -- --config configs/btc_perp.toml`
- **Run with custom accounts registry:** `cargo run --release -- --config <file> --accounts-file ~/.config/hyperliquid/accounts.toml`
- **Dry run:** `cargo run --release -- --config <file> --dry-run`
- **Create config interactively:** `cargo run --release -- --create`
- **List strategies:** `cargo run --release -- --list-strategies`
- **Tests:** `cargo test`
- **Lint:** `cargo clippy`
- **Format:** `cargo fmt`
- **Code quality (MUST run before commits):** `./check_code.sh` (fmt + clippy + test + check)
- **Deploy:** `./deployment/start.sh --config <config_path>` (builds, confirms, runs in tmux)
- **Stop:** `./deployment/stop.sh`

## Config Files
- Strategy configs: TOML in `configs/` (`name`, `account`, `[strategy]`)
- Account registry: `~/.config/hyperliquid/accounts.toml` by default, or `--accounts-file <path>`
- Account secrets: API wallet private keys stored inline in the account registry; keep that file permissioned tightly

### Strategy Config Examples
```toml
# Accounts registry
[accounts.spot_account]
network = "mainnet"
master_account_address = "0xMasterAccountAddress..."
sub_account_address = "0xSpotSubAccountAddress..."
api_wallet_private_key = "0xYourSpotApiWalletPrivateKey"

# Spot Grid
name = "hype-spot-grid"
account = "spot_account"

[strategy]
type = "spot_grid"
symbol = "HYPE/USDC"
grid_range_high = 20.0
grid_range_low = 10.0
grid_count = 50
total_investment = 1000.0
grid_type = "arithmetic"  # or "geometric"

# Perp Grid
name = "btc-perp-grid"
account = "perp_account"

[strategy]
type = "perp_grid"
symbol = "BTC"
leverage = 10
grid_range_high = 89500.0
grid_range_low = 87000.0
grid_type = "geometric"
grid_count = 20
total_investment = 8000.0
grid_bias = "short"  # or "long"
```

## Code Style Rules
- All `Result`s must be handled. Avoid `unwrap()` in critical paths.
- Strategies must never call exchange APIs -- use StrategyContext only.
- Do not commit or modify real secrets under `~/.config/hyperliquid/`.
- Documentation updates required when changing behavior (see `docs/`).
- `schema/bot-ws-schema/schema/events.json` is the Single Source of Truth for WebSocket events.

## Directory Structure
```
src/
├── main.rs                # Entry point (CLI, config loading)
├── lib.rs                 # Library exports
├── model.rs               # Core data types (Cloid, OrderFill, OrderSide)
├── error.rs               # Error types
├── constants.rs           # Tunable parameters
├── engine/
│   ├── live.rs            # Live trading engine (977 lines)
│   ├── simulation.rs      # Dry-run simulation
│   ├── common.rs          # Shared utilities
│   └── context.rs         # Strategy execution context
├── strategy/
│   ├── mod.rs             # Strategy trait & factory
│   ├── spot_grid.rs       # Spot grid implementation
│   ├── perp_grid.rs       # Perp grid implementation
│   ├── common.rs          # Grid calculation utilities
│   └── types.rs           # Strategy types & enums
├── broadcast/
│   ├── server.rs          # WebSocket server (port 9000)
│   └── types.rs           # Event type definitions
├── config/
│   ├── mod.rs             # Config loading
│   ├── strategy.rs        # Strategy config schemas + validation
│   ├── exchange.rs        # Exchange configuration
│   ├── broadcast.rs       # Broadcast config
│   ├── simulation.rs      # Simulation mode config
│   └── creator.rs         # Interactive config wizard
├── logging/
│   └── order_audit.rs     # CSV audit logger
└── ui/
    └── console.rs         # Console display for dry-run
```

## Key Dependencies
- `hyperliquid_rust_sdk` (v0.6.0) -- Exchange interaction
- `tokio` -- Async runtime
- `tokio-tungstenite` (v0.20) -- WebSocket server
- `ethers` (v2.0.14) -- Wallet and signing
- `tracing` + `tracing-subscriber` -- Structured logging with file rotation
- `serde` + `toml` -- Config parsing
- `clap` (v4.0) -- CLI argument parsing
- `dialoguer` (v0.12) -- Interactive config wizard

## Documentation
- `README.md` -- Setup, usage, features
- `DEPLOYMENT.md` -- Production deployment with tmux
- `AGENT.md` -- Stable agent instructions & constraints
- `docs/design.md` -- System architecture, components, data flow
- `docs/strategies/spot_grid.md` -- Spot grid strategy details
- `docs/strategies/perp_grid.md` -- Perp grid strategy details
- `docs/api/websocket_events.md` -- WebSocket event types and payloads

## Related Repos
- **lighter-trading-bot** -- Parallel bot for Lighter.xyz DEX (Python, same strategy concepts)
- **bot-ws-schema** -- Shared WebSocket event schema (git submodule at `schema/bot-ws-schema/`)
- **bot-dashboard** -- React/Electron real-time dashboard (consumes WS events)
