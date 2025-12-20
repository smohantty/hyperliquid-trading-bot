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
*   Wallet Private Key (for signing transactions)

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

3.  Configure Environment:
    Create a `.env` file in the root directory:
    ```env
    WALLET_PRIVATE_KEY=0x...
    SERVICE_ADDRESS=0x...
    ```

### Running the Bot

Run the bot using `cargo run`. You must specify a configuration file.

```bash
# General Usage
cargo run --release -- --config <PATH_TO_CONFIG> [OPTIONS]
```

#### Examples

**1. Run a Spot Grid Strategy:**
```bash
cargo run --release -- --config configs/spot_grid.toml
```

**2. Run with specific WebSocket Port:**
By default, the status server runs on port `9000`. You can change this:
```bash
cargo run --release -- --config configs/my_perp.toml --ws-port 8080
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
**Example `spot_grid.toml`**:
```toml
[strategy]
type = "SpotGrid"
symbol = "HYPE/USDC"
upper_price = 20.0
lower_price = 10.0
grid_count = 50
total_investment = 1000.0
grid_type = "Arithmetic"
# trigger_price = 15.0 # Optional start trigger
```

See [Spot Grid Docs](docs/strategies/spot_grid.md) for full parameter details.

## Real-Time Monitoring

The bot exposes a WebSocket feed at `ws://localhost:<PORT>`.
New connections immediately receive the Strategy Configuration and the latest Status Summary.

**Event Types**:
*   `config`: Strategy settings.
*   `summary`: Periodic snapshots (PnL, Inventory, Zones).
*   `order_update`: Real-time order fills/placements.
*   `market_update`: Price ticks.

## Telegram Integration 📱
The bot can send real-time notifications to your Telegram.

### Setup
1.  **Create a Bot**: Talk to [@BotFather](https://t.me/botfather) on Telegram to create a new bot and get a **Token**.
2.  **Get Chat ID**: Start your bot and send a message. Visit `https://api.telegram.org/bot<YOUR_TOKEN>/getUpdates` to find your `chat.id`.
3.  **Update `.env`**:
    ```env
    TELEGRAM_BOT_TOKEN=123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11
    TELEGRAM_CHAT_ID=123456789
    ```

### Usage
*   **Notifications**: The bot sends a message automatically when an order is **FILLED**.
*   **Commands**: Send `/status` to the bot to get a current snapshot of PnL, Price, and Inventory.

## License
MIT
