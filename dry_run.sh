#!/bin/bash
# Dry Run Simulation
#
# Usage: ./dry_run.sh [config_file]
#        ./dry_run.sh --config path/to/config.toml
#
# Examples:
#   ./dry_run.sh                                             # Uses default strategy config
#   ./dry_run.sh configs/hype_spot_geometric_20_24_40.toml   # Positional config
#   ./dry_run.sh --config configs/hype_spot_geometric_20_24_40.toml  # Named config
#
# Simulation settings come from the optional [simulation] block in the strategy config.

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo "=========================================="
echo " SIMULATION MODE"
echo "=========================================="

# If no args provided, use default config
if [ $# -eq 0 ]; then
    echo "Strategy: configs/hype_spot.toml (default)"
    echo ""
    exec cargo run --locked --release -- --dry-run --config configs/hype_spot.toml
else
    echo ""
    exec cargo run --locked --release -- --dry-run --config "$@"
fi
