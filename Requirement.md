That is a perfect addition for a professional CLI tool. Using **`clap`** (Command Line Argument Parser) will allow you to pass the config path as an argument, provide a `--help` menu, and ensure the bot doesn't just crash silently if a file is missing.

I have integrated **`clap`** into the final Phase 1 `requirements.md` below.

---

# Requirements: Hyperliquid Rust Bot (Phase 1)

## 1. Project Overview

The objective is to build a robust, modular foundation for a Hyperliquid trading bot in Rust. Phase 1 focuses on the **Configuration System**, **CLI Entry Point**, and **Strategy Factory**. The bot must parse command-line arguments to find a TOML config, validate it, and instantiate the corresponding strategy.

## 2. Technical Stack

* **Language:** Rust (Latest Stable)
* **CLI Parser:** `clap` (v4 with `derive` features)
* **Serialization:** `serde` (with `derive` features)
* **Config Format:** `toml`
* **Error Handling:** - `thiserror`: For domain-specific errors (e.g., `ValidationError`).
* `anyhow`: For top-level application error reporting.



## 3. Folder Structure

```text
/
├── Cargo.toml
├── configs/                 # Directory for different strategy tomls
│   ├── eth_spot.toml
│   └── btc_perp.toml
└── src/
    ├── main.rs              # CLI Entry Point (Clap logic)
    ├── lib.rs               # Library root
    ├── error.rs             # BotError enum (thiserror)
    ├── config/
    │   ├── mod.rs           # Loader & Validation logic
    │   └── strategy.rs      # Tagged StrategyConfig Enums
    └── strategy/
        ├── mod.rs           # Strategy Trait & Factory Logic
        ├── spot_grid.rs     
        └── perp_grid.rs     

```

## 4. Functional Specifications

### 4.1. Command Line Interface (Clap)

The bot must implement a CLI with the following:

* **Argument**: `--config` or `-c` (Required). Specifies the path to the strategy TOML file.
* **Help**: Automatic help generation via `#[derive(Parser)]`.
* **Behavior**: If the config file is missing or invalid, the bot must exit with an error message and usage instructions.

### 4.2. Configuration Mapping (Tagged Enum)

The TOML file must use an internal `type` tag to determine the struct:

* **`spot_grid`**: Fields: `symbol`, `upper_price`, `lower_price`, `grid_count`, `per_grid_amount`.
* **`perp_grid`**: Fields: `symbol`, `leverage` (u32), `is_isolated` (bool), `grid_count`, `range_percent`.

### 4.3. The Strategy Factory & Trait

* **Trait**: `Strategy` with `fn run(&self) -> Result<(), BotError>`.
* **Factory**: A function that maps `StrategyConfig` enum variants to a `Box<dyn Strategy>`.

### 4.4. Validation Rules

The system must validate data immediately after parsing:

* `grid_count` must be .
* `spot_grid`: `upper_price` > `lower_price`.
* `perp_grid`: `leverage` must be within .

## 5. Implementation Roadmap for Agent

1. **Cargo.toml**: Add `clap = { version = "4.0", features = ["derive"] }`, `serde`, `toml`, `thiserror`, `anyhow`.
2. **`error.rs`**: Define `BotError` for `ConfigReadError`, `ParsingError`, and `ValidationError`.
3. **`config/strategy.rs`**: Implement the `StrategyConfig` enum with `#[serde(tag = "type")]`.
4. **`strategy/mod.rs`**: Define the `Strategy` trait and a factory function `init_strategy(cfg: StrategyConfig) -> Box<dyn Strategy>`.
5. **`main.rs`**:
* Define a `Args` struct using `clap`.
* Load the file path from args.
* Pass the path to the config loader.
* Pass the resulting config to the strategy factory.
* Call `.run()` on the returned strategy.



---

### Ready to Code?

This `requirements.md` is now complete for a coding agent to generate the full Phase 1 skeleton.

**Would you like me to generate the `Cargo.toml` and the `main.rs` with the `clap` setup to get you started?**