#!/bin/bash
set -e

SESSION_NAME="hyperliquid-bot"
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."

# Default config path (relative to project root)
CONFIG_PATH="configs/eth_perp_grid.toml"

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --config) CONFIG_PATH="$2"; shift ;;
        --skip-build) SKIP_BUILD=1 ;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

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
if [ -z "$SKIP_BUILD" ]; then
    echo "Building release binary..."
    cargo build --release
    if [ $? -ne 0 ]; then
        echo "Build failed. Exiting."
        exit 1
    fi
fi

# Path to binary
BINARY="./target/release/hyperliquid-trading-bot"

if [ ! -f "$BINARY" ]; then
    echo "Binary not found at $BINARY. Creating it now..."
    cargo build --release
fi

echo "Starting new tmux session '$SESSION_NAME' with config: $CONFIG_PATH"
# Create a new detached session
tmux new-session -d -s "$SESSION_NAME"

# Run the bot in the session
# We use 'exec' so the pane closes if the bot crashes, or we can keep it open with a shell loop
tmux send-keys -t "$SESSION_NAME" "$BINARY --config $CONFIG_PATH" C-m

echo "Bot started in background."
echo "View logs/process with: tmux attach -t $SESSION_NAME"
echo "To detach again, press: Ctrl+b, then d"
