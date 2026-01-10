# Hyperliquid Trading Bot: Agent Instructions

## 1. Project Context
You are working on a high-performance, event-driven trading bot for Hyperliquid (Spot & Perp).
The codebase is written in **Rust** and uses `tokio` for concurrency.
Key dependencies:
-   `hyperliquid_rust_sdk` for exchange interaction.
-   `teloxide` for Telegram integration.
-   `tracing` for structured logging.

### System Architecture Summary
*   **Engine** (`src/engine`): The "brain". Orchestrates the event loop, manages Exchange connection (`hyperliquid_rust_sdk`), state, and error handling.
*   **Strategy** (`src/strategy`): Pure logic. Receives `on_tick` and `on_order_filled` events. Returns simple actions. **DO NOT** make API calls here.
*   **Broadcaster** (`src/broadcast`): Sidecar WebSocket server. Pushes state to the Frontend/UI.
*   **Frontend** (`frontend/`): Vite + Electron app for visualization.
*   **Models** (`src/model.rs`): Shared types (`Cloid`, `OrderRequest`, `OrderFill`) to decouple logic from the raw SDK.

### Developer Cheat Sheet
*   **Add Strategy**: Implement `Strategy` trait in `src/strategy/`. Register in `src/config/strategy.rs` and `src/strategy/mod.rs`.
*   **Config**: Defined in `src/config/strategy.rs`. Supports `SpotGrid` and `PerpGrid`. 
    *   *Validation logic is critical*: Check `validate()` in `strategy.rs`.
*   **Run Local**: `cargo run --bin hyperliquid-trading-bot -- -c configs/config.toml`
*   **Run Frontend**: `cd frontend && npm install && npm run dev`

### Critical Documentation
Before starting any task, **YOU MUST READ** these documents to understand the system:
-   **Architecture**: `docs/design.md` (System overview, components, data flow).
-   **Strategies**: `docs/strategies/` (Detailed logic for `SpotGrid` and `PerpGrid`).
-   **Usage**: `README.md` (Setup, CLI commands, Config).
-   **Config Reference**: `src/config/strategy.rs` (The authoritative definition of valid parameters).

## 2. Mandatory Workflow
You must strictly follow this process for every user request involving code changes:

### Phase 1: Requirements Gathering
1.  **Analyze**: Read the User Request.
2.  **Document**: Create or Update `requirements.md` (in the root or strictly associated with the task).
    *   Define *what* needs to be done.
    *   Identify edge cases.
    *   List parameters to change.

### Phase 2: Planning
1.  **Plan**: Create `implementation_plan.md`.
    *   Link to the Requirements.
    *   List specific files to modify.
    *   Describe the logic changes (pseudo-code if complex).
    *   Define verification steps (Tests, Manual checks).
2.  **Review**: **STOP and Ask the User** to review the Plan. Do *not* write code until the plan is approved.

### Phase 3: Execution
1.  **Implement**: Write the code according to the approved plan.
2.  **Documentation**:
    *   **CRITICAL**: If you change behavior, add a feature, or modify a strategy, you **MUST** update the relevant documentation in `docs/` or `README.md` immediately. Code and Docs must never drift apart.
    *   **API/Events**: `docs/api/schema.json` is the **Single Source of Truth** for the WebSocket API. If data usage changes, update this schema first. Strategy docs should link to it.

### Phase 4: Verification
1.  **Automated Check**: Run `./check_code.sh`.
    *   This script runs `cargo fmt`, `cargo clippy`, `cargo test`, and `cargo check`.
    *   **Mandatory**: This *must* pass before task completion.
2.  **Verify**: Confirm the change meets the Requirements.

## 3. System Architecture Constraints
*   **Engine vs Strategy**: The `Engine` (`src/engine/`) manages connections and state. The `Strategy` (`src/strategy/`) is pure logic.
    *   *Never* put networking/API calls inside a Strategy.
    *   Strategies process `on_tick` and `on_order_filled` and return standard actions.
*   **Broadcasting**: Status updates should be sent via the `StatusBroadcaster` (`src/broadcast/`).
*   **Safety**: All `Result`s must be handled. Avoid `unwrap()` in critical paths.
*   **Telegram**: Be aware of the `TelegramReporter`. If you change `StatusSummary`, check if it affects the Telegram `/status` output or notifications. Keep the reporter robust (don't let it crash the Engine).

## 4. Key Locations
*   **Config**: `src/config/`
*   **Engine**: `src/engine/`
*   **Strategies**: `src/strategy/`
*   **Broadcasting**: `src/broadcast/`
*   **Reporters**: `src/reporter/` (Telegram)
*   **Logging**: `src/logging/` (Audit)
*   **Core Types**: `src/model.rs`
*   **Frontend**: `frontend/` (Vite + Electron)
*   **Docs**: `docs/`

Remember: **Documentation is part of the Code.** Update it.
