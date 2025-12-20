# Requirements


## Bot Status Broadcasting (WebSocket)

To enable visualizing the bot's state on any frontend (Web, Telegram, CLI), the bot will host a lightweight WebSocket server to broadcast real-time updates.

### 1. Data Protocol
*   **Format**: JSON.
*   **Structure**: All messages are wrapped in a standard Envelope.
    ```json
    {
      "type": "EventType",
      "ts": "ISO-8601 or UnixMs",
      "payload": { ... }
    }
    ```

### 2. Event Types

#### A. `config` (On Connect/Change)
*   Broadcasts the static configuration of the running strategy.
*   **Payload**: The serialized TOML config (e.g., `SpotGridConfig`). Allows frontend to see "What are we running?".

#### B. `summary` (Periodic/On-Change)
Broadcasts dynamic status.
*   **General Stats**:
    *   `pnl`: { `realized`: f64, `unrealized`: f64, `total_fees`: f64 }
    *   `inventory`: { `size`: f64, `avg_entry_price`: f64 }
    *   `wallet`: { `base`: f64, `quote`: f64 }
    *   `market`: { `current_price`: f64 }
*   **Strategy-Specific (`zones`)**:
    *   List of Grid Zones: `[{ price: 100.0, side: "Buy", status: "Open" }, { price: 101.0, side: "Sell", status: "Filled" }]`.
    *   This explicitly supports the CLOB-style visualization the user requested.

#### C. `order_fill` (Real-time)
Broadcasts when an order is filled.
*   Fields: `side`, `price`, `size`, `role` (Maker/Taker), `fee`.

### 3. Server Configuration
*   **Method**: Command Line Argument.
    *   `--ws-port <PORT>`
*   **Default**: `9000` (if argument not provided).
*   **Disable**: Setting port to `0` or a specific flag (e.g. `--no-ws`) could disable it, but for now we'll just assume it runs unless port binding fails.

### 4. Implementation Details
*   **Technology**: `tokio-tungstenite` for the server.
*   **Concurrency**: The server runs in a separate generic Tokio task. The `Engine` pushes updates to it via a `broadcast` channel (MPMC).
*   **Trait Extension**: Strategies must implement a method `get_status_snapshot() -> serde_json::Value` to provide custom data for the `summary` event.
