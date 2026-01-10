#!/bin/bash
set -e

echo "Running cargo fmt..."
cargo fmt

echo "Running cargo clippy..."
cargo clippy -- -D warnings

echo "Running cargo test..."
cargo test

echo "Running cargo check..."
cargo check

echo "All checks passed!"
