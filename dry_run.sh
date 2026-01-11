#!/bin/bash
# Dry Run Simulation
#
# Usage: ./dry_run.sh [config_file]
#        ./dry_run.sh --config path/to/config.toml
#
# Examples:
#   ./dry_run.sh                             # Uses default strategy config
#   ./dry_run.sh configs/spot_HYPE.toml      # Positional config
#   ./dry_run.sh --config configs/spot_HYPE.toml  # Named config
#
# Simulation configuration is read from HYPERLIQUID_SIMULATION_CONFIG_FILE in .env
# (default: simulation_config.json)

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo "=========================================="
echo " SIMULATION MODE"
echo "=========================================="

# If no args provided, use default config
if [ $# -eq 0 ]; then
    echo "Strategy: configs/spot_HYPE.toml (default)"
    echo ""
    exec cargo run -- --dry-run --config configs/spot_HYPE.toml
else
    echo ""
    exec cargo run -- --dry-run "$@"
fi
