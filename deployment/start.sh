#!/bin/bash
set -euo pipefail

SESSION_NAME="hyperliquid-bot"
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."

CONFIG_PATH=""
ACCOUNTS_FILE=""
SKIP_BUILD=0

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --config)
            if [[ -z "${2:-}" ]]; then
                echo "Error: --config requires a file path."
                exit 1
            fi
            CONFIG_PATH="$2"
            shift
            ;;
        --accounts-file)
            if [[ -z "${2:-}" ]]; then
                echo "Error: --accounts-file requires a file path."
                exit 1
            fi
            ACCOUNTS_FILE="$2"
            shift
            ;;
        --skip-build) SKIP_BUILD=1 ;;
        *)
            if [[ -z "$CONFIG_PATH" ]]; then
                CONFIG_PATH="$1"
            else
                echo "Unknown parameter passed: $1"
                exit 1
            fi
            ;;
    esac
    shift
done

if [[ -z "$CONFIG_PATH" ]]; then
    echo "Error: You must provide a strategy config path."
    echo "Usage:"
    echo "  ./deployment/start.sh configs/my_strategy.toml [--skip-build] [--accounts-file PATH]"
    echo "  ./deployment/start.sh --config configs/my_strategy.toml [--skip-build] [--accounts-file PATH]"
    exit 1
fi

# Check if tmux is installed
if ! command -v tmux &> /dev/null; then
    echo "Error: tmux is not installed. Please install it first."
    exit 1
fi

cd "$PROJECT_ROOT"

# Check if session exists
if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    echo "Session '$SESSION_NAME' already exists."
    echo "Attach with: tmux attach -t $SESSION_NAME"
    exit 0
fi

# Build release binary unless skipped
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    echo "Building release binary..."
    cargo build --release
fi

# Path to binary
BINARY="./target/release/hyperliquid-trading-bot"

if [[ ! -f "$BINARY" ]]; then
    echo "Binary not found at $BINARY. Creating it now..."
    cargo build --release
fi

RUN_ARGS=(--config "$CONFIG_PATH")
if [[ -n "$ACCOUNTS_FILE" ]]; then
    RUN_ARGS+=(--accounts-file "$ACCOUNTS_FILE")
fi

echo "Running Dry Run Simulation..."
if ! "$BINARY" --dry-run "${RUN_ARGS[@]}"; then
    echo "Dry run failed. Aborting live deployment."
    exit 1
fi

echo ""
read -p "Do you want to proceed with live deployment? (y/N) " -n 1 -r
echo ""
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Deployment aborted."
    exit 0
fi

printf -v TMUX_CMD 'exec %q ' "$BINARY" "${RUN_ARGS[@]}"

echo "Starting new tmux session '$SESSION_NAME' with config: $CONFIG_PATH"
tmux new-session -d -s "$SESSION_NAME" -c "$PROJECT_ROOT" "$TMUX_CMD"

echo "Bot started in background."
echo "View logs/process with: tmux attach -t $SESSION_NAME"
echo "To detach again, press: Ctrl+b, then d"
