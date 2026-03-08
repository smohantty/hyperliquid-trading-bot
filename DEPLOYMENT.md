# Deployment Guide

This guide describes how to deploy the Hyperliquid Trading Bot using `tmux`. This method ensures the bot keeps running after you disconnect from the server/terminal, but requires manual startup (it will **not** auto-start on boot).

## Prerequisites
- **tmux**: Must be installed (`sudo apt install tmux` on Ubuntu/Debian, or `brew install tmux` on macOS).
- **Rust**: The valid toolchain to build the bot.

## Quick Start

### 1. Start the Bot
Run the start script with a strategy config path. It builds the project, runs a foreground dry-run preflight with live market/account data, prompts for confirmation, and then starts the live bot in `tmux`.

```bash
./deployment/start.sh configs/my_strategy.toml
```

**Options:**
- Pass the strategy config explicitly with `--config` if you prefer:
  ```bash
  ./deployment/start.sh --config configs/my_custom_config.toml
  ```
- Override the accounts registry path:
  ```bash
  ./deployment/start.sh configs/my_custom_config.toml --accounts-file ~/.config/hyperliquid/accounts.toml
  ```
- Skip the build step (faster restart):
  ```bash
  ./deployment/start.sh configs/my_custom_config.toml --skip-build
  ```

If the dry-run fails, the live deployment is aborted automatically.

### 2. View the Bot
To see what the bot is doing (logs, status):

```bash
tmux attach -t hyperliquid-bot
```

**To Detach (Exit view without stopping bot):**
Press `Ctrl+b`, then press `d`.

### 3. Stop the Bot
Gracefully stop the bot and close the session:

```bash
./deployment/stop.sh
```

## Troubleshooting

- **Session not found**: The bot might have crashed immediately. Try running without tmux to debug:
  ```bash
  cargo run --release -- --config configs/eth_perp_grid.toml
  ```
- **Dry-run failed**: Fix the reported configuration, balance, or exchange setup issue first. The deployment script will not continue to live mode after a failed dry-run.
- **"Address already in use"**: Ensure no other instance is running. The `stop.sh` script attempts to kill the session, but check manually with `ps aux | grep hyperliquid` if issues persist.
