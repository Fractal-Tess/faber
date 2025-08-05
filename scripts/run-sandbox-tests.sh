#!/bin/sh

# Script to run sandbox crate unit tests in Docker environment
# This script assumes you're running in a Docker container with appropriate privileges

set -e

echo "Running sandbox crate unit tests..."

# Change to the project root
cd /home/fractal-tess/dev/faber

# Build the project first (as per user preferences)
echo "Building project..."
cargo build

# Run tests for the sandbox crate specifically
echo "Running sandbox tests..."
cargo test -p faber-container

# Run tests with output (to see test results)
echo "Running sandbox tests with output..."
cargo test -p faber-container -- --nocapture

# Run specific test modules if needed
echo "Running specific test modules..."
cargo test -p faber-container error_tests -- --nocapture
cargo test -p faber-container resource_limits_tests -- --nocapture
cargo test -p faber-container namespace_settings_tests -- --nocapture
cargo test -p faber-container container_config_tests -- --nocapture

echo "All sandbox tests completed!" 