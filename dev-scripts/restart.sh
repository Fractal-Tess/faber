#!/usr/bin/env bash

# Development restart script for Faber
# This script restarts the Faber process in the development container

set -e

CONTAINER_NAME="faber-dev"

echo "🔄 Restarting Faber process in development container..."

# Check if container is running
if ! docker ps --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
    echo "❌ Container ${CONTAINER_NAME} is not running"
    echo "Start it with: docker compose -f docker-compose.dev.yml up -d"
    exit 1
fi

# Restart the Faber process using supervisor
echo "📦 Restarting Faber process..."
docker exec ${CONTAINER_NAME} supervisorctl restart faber

# Wait a moment for the process to start
sleep 2

# Check if the process is running
if docker exec ${CONTAINER_NAME} supervisorctl status faber | grep -q "RUNNING"; then
    echo "✅ Faber process restarted successfully"
    echo "📊 Process status:"
    docker exec ${CONTAINER_NAME} supervisorctl status faber
else
    echo "❌ Failed to restart Faber process"
    echo "📋 Recent logs:"
    docker exec ${CONTAINER_NAME} tail -20 /var/log/supervisor/faber.log
    exit 1
fi

echo "🚀 Ready for testing!"
echo "Test with: curl http://localhost:3000/health" 