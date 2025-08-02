#!/bin/bash

# Development script for Faber
export RUST_LOG=debug
export LOG_FORMAT=text
export PORT=3000
export HOST=0.0.0.0

echo "Starting Faber in development mode..."
echo "Log level: $RUST_LOG"
echo "Port: $PORT"
echo "Host: $HOST"

# Check if cargo-watch is installed
if ! command -v cargo-watch &> /dev/null; then
    echo "Installing cargo-watch..."
    cargo install cargo-watch
fi

# Run with hot reloading
cargo watch -x run 