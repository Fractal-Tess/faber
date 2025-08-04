#!/bin/bash

# Convenience script for setting up remote debugging
# This script automates the entire debugging setup process

set -e

echo "🚀 Setting up remote debugging for Faber..."

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Cargo.toml not found. Please run this script from the project root."
    exit 1
fi

# Build the application
echo "📦 Building application..."
cargo build

if [ $? -ne 0 ]; then
    echo "❌ Build failed. Please fix the build errors and try again."
    exit 1
fi

echo "✅ Build completed successfully"

# Start Docker container
echo "🐳 Starting Docker container..."
docker-compose up -d

# Wait a moment for container to be ready
sleep 3

# Check if container is running
if ! docker-compose ps | grep -q "Up"; then
    echo "❌ Container failed to start. Check docker-compose logs."
    exit 1
fi

echo "✅ Container is running"

# Start debug server in container
echo "🔧 Starting debug server in container..."
docker-compose exec -d faber lldb-server platform --server --listen *:12345

# Wait for debug server to start
sleep 2

# Check if debug server is running
if docker-compose exec faber ps aux | grep -q "lldb-server"; then
    echo "✅ Debug server is running on port 12345"
else
    echo "❌ Debug server failed to start"
    exit 1
fi

echo ""
echo "🎉 Remote debugging setup complete!"
echo ""
echo "Next steps:"
echo "1. Open VSCode in this directory"
echo "2. Go to Run and Debug panel (Ctrl+Shift+D)"
echo "3. Select 'Remote Debug (Docker Container)'"
echo "4. Press F5 to start debugging"
echo ""
echo "To stop debugging:"
echo "  docker-compose exec faber pkill lldb-server"
echo "  docker-compose down"
echo ""
echo "To view container logs:"
echo "  docker-compose logs -f faber" 