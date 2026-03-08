# Hyperliquid Trading Bot

An advanced, event-driven trading bot for the Hyperliquid exchange, written in Rust. It supports high-frequency Grid strategies for both Spot and Perpetual markets with real-time status broadcasting.

## Features
*   **High Performance**: Built on `tokio` for non-blocking I/O and low latency.
*   **Dual Markets**: Supports `SpotGrid` and `PerpGrid` (with leverage) strategies.
*   **Live Monitoring**: Native WebSocket server broadcasts state to external UIs (Web/CLI).
*   **Robust Engine**: Safety checks for balances, order tracking, and error handling.
*   **Visual Order Book**: Strategies export zone data for CLOB-style visualizations.

## Documentation
*   [**Architecture Design**](docs/design.md): System overview, component diagrams, and data flow.
*   [**Spot Grid Strategy**](docs/strategies/spot_grid.md): Start here for Spot trading logic.
*   [**Perp Grid Strategy**](docs/strategies/perp_grid.md): Advanced grid logic for Perpetuals.

## Getting Started

### Prerequisites
*   Rust (latest stable)
*   A Hyperliquid Account (Mainnet or Testnet)
*   **Accounts Registry**: A TOML file outside the repo that stores named account profiles.

### Installation
1.  Clone the repository:
    ```bash
    git clone https://github.com/your-username/hyperliquid-trading-bot.git
    cd hyperliquid-trading-bot
    ```

2.  Build the project:
    ```bash
    cargo build --release
    ```

3.  Create your accounts registry at the default location:
    ```text
    ~/.config/hyperliquid/accounts.toml
    ```

    Template:
    [configs/accounts.template.toml](configs/accounts.template.toml)

    Notes:
    - `sub_account_address` is optional. If omitted, the bot trades the master account.
    - The private key must belong to an API wallet approved by your Hyperliquid master account.
    - The same API wallet can sign for a subaccount, but use a separate API wallet per live bot process to avoid nonce collisions.
    - Because `accounts.toml` now contains secrets, keep file permissions tight, for example `chmod 600 ~/.config/hyperliquid/accounts.toml`.
    - Do not use your master EOA private key.

### Running the Bot

Run the bot using `cargo run`. You must specify a strategy configuration file. The bot does not read runtime config from a `.env` file.

```bash
# General Usage
cargo run --release -- --config <PATH_TO_STRATEGY_CONFIG> [OPTIONS]

# Optional: override the accounts registry path
cargo run --release -- --config <PATH_TO_STRATEGY_CONFIG> --accounts-file <PATH_TO_ACCOUNTS_TOML>
```

## Deployment (Production)

For long-running production usage, we recommend using our `tmux` based deployment scripts which allow the bot to persist after you close your terminal.

See using [**Deployment Guide**](DEPLOYMENT.md) for full details.

### Quick Start
```bash
./deployment/start.sh configs/<your_strategy>.toml
```

The deployment start script runs a dry-run preflight first, prints the simulation output in the terminal, and asks for confirmation before launching live trading.

#### Examples

**1. Run a Spot Grid Strategy:**
```bash
cargo run --release -- --config configs/hype_spot_geometric_20_24_40.toml
```

**2. Run with a bot-specific WebSocket Port:**
Set `websocket_port` in the strategy config. If omitted, it defaults to `8000` for spot strategies and `8001` for perp strategies.
```bash
cargo run --release -- --config configs/btc_perp.toml
```

## Configuration

Strategies are defined in `.toml` files. You can create them manually or use the interactive wizard.

### Interactive Creation
The bot includes a wizard to guide you through creating a valid configuration file.

```bash
cargo run --release -- --create
```
Follow the on-screen prompts to select your strategy type and parameters. The file will be saved to your specified path.

### Manual Configuration
**Example strategy config**:
```toml
name = "hype-spot-grid"
account = "spot_account"
# websocket_port = 8100 # Optional, defaults to 8000 for spot and 8001 for perp

[simulation]
USDC = 5000.0
HYPE = 100.0

[strategy]
type = "spot_grid"
symbol = "HYPE/USDC"
grid_range_high = 20.0
grid_range_low = 10.0
grid_type = "arithmetic"
grid_count = 50
# spread_bips = 50.0 # Use this instead of grid_count
total_investment = 1000.0
# trigger_price = 15.0 # Optional start trigger
```

The `[simulation]` block is optional and only affects `--dry-run`. Dry-run always uses live market data and real account balances. If the block contains asset values, those balances are applied on top of the fetched account state.
For grid spacing, use either `grid_count` or `spread_bips`. `grid_type` remains part of the strategy config and defaults to `geometric` when omitted. When `spread_bips` is used, spacing is geometric by definition, so `grid_type` must remain `geometric`.

See [Spot Grid Docs](docs/strategies/spot_grid.md) for full parameter details.

## Real-Time Monitoring

The bot exposes a WebSocket feed at `ws://localhost:<PORT>`.
New connections immediately receive the Strategy Configuration and the latest Status Summary.

**Event Types**:
*   `config`: Strategy settings.
*   `summary`: Periodic snapshots (PnL, Inventory, Zones).
*   `order_update`: Real-time order fills/placements.
*   `market_update`: Price ticks.

## Dashboard Web App

A modern React-based dashboard is included to visualize the bot's status in real-time.

### Running the Dashboard

1.  **Start the Bot** (Ensure it's running and broadcasting):
    ```bash
    cargo run --release -- --config configs/<your_strategy>.toml
    ```

2.  **Start the Web Client**:
    Open a new terminal in the `frontend` directory:
    ```bash
    cd frontend
    npm install  # First time only
    npm run dev
    ```

3.  **Access the Interface**:
    Open your browser and navigate to `http://localhost:5173`.

### Desktop App (Electron)

You can also run the dashboard as a standalone desktop application.

**Build:**
```bash
cd frontend
npm run electron:build
```
The executable (e.g., `.AppImage`) will be generated in `frontend/dist`.

**Run:**
Simply execute the generated binary:
```bash
./frontend/dist/hyperliquid-dashboard-0.0.0.AppImage
```

### Features
*   **Real-time PnL & Balance Tracking**
*   **Live Grid Visualization** (Order Book style)
*   **Active Strategy Configuration**
*   **Connection Status Indicator**

## License
MIT
