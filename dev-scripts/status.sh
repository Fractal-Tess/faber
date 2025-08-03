#!/usr/bin/env bash

# Development status script for Faber
# This script shows the status of the development container and process

CONTAINER_NAME="faber-dev"

echo "📊 Faber Development Container Status"
echo "====================================="

# Check if container is running
if docker ps --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
    echo "✅ Container Status: RUNNING"
    echo ""
    
    # Show container details
    echo "📦 Container Details:"
    docker ps --filter "name=${CONTAINER_NAME}" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
    echo ""
    
    # Show process status
    echo "🔄 Process Status:"
    docker exec ${CONTAINER_NAME} supervisorctl status faber
    echo ""
    
    # Show recent logs
    echo "📋 Recent Logs (last 10 lines):"
    docker exec ${CONTAINER_NAME} tail -10 /var/log/supervisor/faber.log
    echo ""
    
    # Test health endpoint
    echo "🏥 Health Check:"
    if curl -s http://localhost:3000/health > /dev/null; then
        echo "✅ Health endpoint responding"
        curl -s http://localhost:3000/health | jq . 2>/dev/null || curl -s http://localhost:3000/health
    else
        echo "❌ Health endpoint not responding"
    fi
    
else
    echo "❌ Container Status: NOT RUNNING"
    echo ""
    echo "To start the development container:"
    echo "  docker compose -f docker-compose.dev.yml up -d"
    echo ""
    echo "To build and start:"
    echo "  docker compose -f docker-compose.dev.yml up --build -d"
fi 