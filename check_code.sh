#!/bin/bash
set -e

echo "Running cargo fmt..."
cargo fmt

echo "Running cargo clippy..."
cargo clippy --locked -- -D warnings

echo "Running cargo test..."
cargo test --locked

echo "Running cargo check..."
cargo check --locked

echo "All checks passed!"
