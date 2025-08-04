#!/bin/bash

# Cleanup script for remote debugging environment

echo "🧹 Cleaning up remote debugging environment..."

# Stop debug server in container
echo "🛑 Stopping debug server..."
docker-compose exec faber pkill lldb-server 2>/dev/null || true

# Stop and remove containers
echo "🐳 Stopping Docker containers..."
docker-compose down

echo "✅ Cleanup complete!"
echo ""
echo "To start debugging again, run:"
echo "  ./dev-scripts/debug.sh" 