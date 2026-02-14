#!/usr/bin/env bash
# Docker Integration Test Runner for Faber
# Builds image, runs container, executes tests, cleans up

set -e

echo "=== Faber Docker Integration Test Runner ==="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Configuration
API_KEY="${API_KEY:-test-key-123}"
PORT="${PORT:-3000}"

# Build the Docker image
echo "[1/4] Building Faber Docker image..."
docker build -f docker/prod/Dockerfile -t faber-test:latest . 2>&1 | tail -10

if [ $? -ne 0 ]; then
    echo -e "${RED}Failed to build Docker image${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Docker image built${NC}"
echo ""

# Setup cgroup hierarchy (required for faber to work)
echo "[2/4] Setting up cgroup hierarchy..."
sudo mkdir -p /sys/fs/cgroup/faber 2>/dev/null || true
sudo chmod 777 /sys/fs/cgroup/faber 2>/dev/null || true
echo "+cpu +memory +pids" | sudo tee /sys/fs/cgroup/faber/cgroup.subtree_control 2>/dev/null || true
echo -e "${GREEN}✓ Cgroup hierarchy ready${NC}"
echo ""

# Run the container in background
echo "[3/4] Starting Faber container..."
CONTAINER_ID=$(sudo docker run -d \
    --privileged \
    --cgroupns=host \
    -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
    -e API_KEY=$API_KEY \
    -e CACHE_ENABLED=true \
    -p $PORT:$PORT \
    faber-test:latest)

if [ -z "$CONTAINER_ID" ]; then
    echo -e "${RED}Failed to start container${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Container started: $CONTAINER_ID${NC}"

# Cleanup function
cleanup() {
    echo ""
    echo "[4/4] Cleaning up..."
    sudo docker stop $CONTAINER_ID > /dev/null 2>&1 || true
    sudo docker rm $CONTAINER_ID > /dev/null 2>&1 || true
    sudo rm -rf /sys/fs/cgroup/faber/task-* 2>/dev/null || true
    echo -e "${GREEN}✓ Cleanup complete${NC}"
}

trap cleanup EXIT

# Run tests
echo ""
API_URL="http://localhost:$PORT/api/v1" API_KEY=$API_KEY ./scripts/comprehensive-test.sh
