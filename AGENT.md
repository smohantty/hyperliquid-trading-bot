# Hyperliquid Trading Bot: Agent Instructions

## 1. Project Context
You are working on a high-performance, event-driven trading bot for Hyperliquid (Spot & Perp).
The codebase is written in **Rust** and uses `tokio` for concurrency.

### Critical Documentation
Before starting any task, **YOU MUST READ** these documents to understand the system:
-   **Architecture**: `docs/design.md` (System overview, components, data flow).
-   **Strategies**: `docs/strategies/` (Detailed logic for `SpotGrid` and `PerpGrid`).
-   **Usage**: `README.md` (Setup, CLI commands, Config).

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
1.  **Compiles?**: Always run `cargo check`.
2.  **Format?**: Run `cargo fmt` to ensure code style compliance.
3.  **Tests?**: Run `cargo test` if applicable.
4.  **Verify**: Confirm the change meets the Requirements.

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
*   **Docs**: `docs/`

Remember: **Documentation is part of the Code.** Update it.
