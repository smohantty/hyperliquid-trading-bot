# Telegram Bot Integration Design

## 1. Overview
The goal is to provide real-time status updates and notifications to a Telegram user from the Hyperliquid Trading Bot.

## 2. Architecture & Responsibility
**Q: How are we going to develop it? Who has responsibility?**

We will introduce a new component: `TelegramReporter`.

*   **Responsibility**: The `TelegramReporter` is responsible for:
    1.  Maintaining the connection to Telegram (Polling or Webhook).
    2.  Listening to internal bot events (specifically `StatusSummary` and `OrderUpdate`).
    3.  Formatting these events into user-friendly text messages.
    4.  Sending messages to the authorized `chat_id`.

*   **Integration Point**:
    *   The `TelegramReporter` should be initialized in `main.rs`, similar to `ExchangeClient` or `StatusBroadcaster`.
    *   It will run as a separate `tokio::task`.
    *   It will consume a `Receiver<BroadcastEvent>` from a broadcast channel (MPSC or Broadcast) that the `Engine` publishes to.

**Recommendation**: Use the `teloxide` crate. It is the most robust, idiomatic, and feature-rich Telegram library for Rust.

## 3. Update Interval
**Q: What should be the interval?**

We need two types of updates:

1.  **Event-Driven (Real-time)**:
    *   **Order Fills**: Immediate notification. "filled 10 HYPE @ 20.0".
    *   **Errors**: Immediate critical alerts.
    *   **Strategy Start/Stop**: Immediate.

2.  **Periodic (Status Pulse)**:
    *   **Hourly**: A summary snapshot (PnL, Position, Price).
    *   **On-Demand**: The user sends `/status` to the bot, and it replies immediately with the snapshot.

## 4. UI Data & Rendering
**Q: What kind of UI data we need to send? Who will render that data?**

Telegram renders text (MarkdownV2/HTML). We cannot send the raw JSON we use for the Frontend.

*   **Data Layout**:
    *   **Header**: Emoji status (🟢 Running, 🔴 Stopped).
    *   **PnL**: Realized/Unrealized PnL (e.g., `PnL: +$45.00 (+$12.50 Unrl)`).
    *   **Position**: Current inventory Size & Entry (e.g., `Pos: 15.0 HYPE @ $19.5`).
    *   **Price**: Current Market Price.
    *   **Zones**: (Optional/Condensed) "Active Zones: 45/50".

*   **Rendering**:
    *   **The Bot (`TelegramReporter`) renders the data.**
    *   It converts the `StatusSummary` struct into a formatted Markdown string.
    *   *Example*:
        ```
        🟢 **SpotGrid: HYPE/USDC**
        💰 **PnL**: $120.50
        📉 **Price**: $21.30
        📦 **Inv**: 500 HYPE ($20.1 avg)
        ```
    *   **Charts**: For V1, we stick to text. Generating images (PNG) of charts in Rust requires heavy dependencies (like `plotters`) and is complex.

## 5. Implementation Plan (High Level)
1.  Add `teloxide` to `Cargo.toml`.
2.  Create `src/reporter/telegram.rs`.
3.  Define `TelegramConfig` (Token, ChatID).
4.  Implement `start_telegram_bot` function that spawns the listener.
5.  Connect `Engine` broadcasting to `TelegramReporter`.
