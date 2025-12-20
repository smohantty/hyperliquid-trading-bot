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
./frontend/dist/web-client-0.0.0.AppImage
```

### Features
*   **Real-time PnL & Balance Tracking**
*   **Live Grid Visualization** (Order Book style)
*   **Active Strategy Configuration**
*   **Connection Status Indicator**

## Telegram Integration 📱
The bot can send real-time notifications to your Telegram.

### Setup
1.  **Create a Bot**: Talk to [@BotFather](https://t.me/botfather) on Telegram to create a new bot and get a **Token**.
2.  **Get Chat ID**:
    *   Open your new bot in Telegram.
    *   Click **Start** or send a message (e.g., "Hello").
    *   Visit `https://api.telegram.org/bot<YOUR_TOKEN>/getUpdates` in your browser.
    *   Look for `"chat":{"id":12345678,...}` in the JSON response. This `12345678` is your `TELEGRAM_CHAT_ID`.
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
