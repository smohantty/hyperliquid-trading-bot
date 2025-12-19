# Hyperliquid Trading Bot Agent Context

## Project Goal
Build a robust, modular, and user-friendly trading bot for the Hyperliquid exchange (Spot and Perpetual markets), written in Rust. The bot supports multiple strategies, configuration via TOML/CLI/Interactive Wizard, and secure credential management.

## Project Structure

### Core
- **`src/main.rs`**: Application entry point. Handles CLI arguments (`clap`), initializes configuration, and starts the selected strategy.
- **`src/lib.rs`**: Crate root, exposing modules.
- **`src/error.rs`**: Centralized error handling using `thiserror` (`BotError`).

### Configuration (`src/config/`)
- **`mod.rs`**: Module definition and config validation logic.
- **`strategy.rs`**: Defines `StrategyConfig` enum (tagged by `type`) and parameter structs (`SpotGrid`, `PerpGrid`). Includes helper methods (`type_name`, `symbol`) and help text generation.
- **`exchange.rs`**: Handles loading secure credentials (`private_key`, `api_key`) and network settings from `.env` using `dotenvy`.
- **`creator.rs`**: Interactive CLI wizard (`dialoguer`) for generating strategy configuration files with smart filename suggestions.

### Strategy Engine (`src/strategy/`)
- **`mod.rs`**: Defines the `Strategy` trait (with `run()` method) and the `init_strategy` factory function.
- **`spot_grid.rs`**: Implementation of the Spot Grid strategy.
- **`perp_grid.rs`**: Implementation of the Perpetual Grid strategy.

## Key Features
1.  **Modular Strategy System**: Easily extensible `Strategy` trait.
2.  **Robust Configuration**:
    -   TOML-based config files.
    -   Strong typing and validation (e.g., `SpotGrid` vs `PerpGrid` params).
    -   Auto-validation of logical constraints (e.g., `upper_price > lower_price`).
3.  **CLI Interface**:
    -   `--config <PATH>`: Run with a specific config.
    -   `--list-strategies`: documentation.
    -   `--create`: Interactive wizard to generate configs.
    -   `--help`: Standard help.
4.  **Security**:
    -   Credentials loaded from `.env` (gitignored).
    -   Never hardcoded in source.

## Development Workflow
-   **Run Bot**: `cargo run -- --config configs/your_config.toml`
-   **Create Config**: `cargo run -- --create`
-   **List Strategies**: `cargo run -- --list-strategies`
-   **Check**: `cargo check`
-   **Test**: `cargo test`

## Current Status
-   **Phase 1-5 Complete**: Core config, strategies (mock logic), CLI, Validation, Interactive Creator, Env support.
-   **Next Steps**: Implement actual trading logic using Hyperliquid SDK (Phase 6+).
