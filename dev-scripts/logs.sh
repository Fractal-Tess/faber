#!/usr/bin/env bash

# Development logs script for Faber
# This script shows logs from the development container

CONTAINER_NAME="faber-dev"

# Check if container is running
if ! docker ps --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
    echo "❌ Container ${CONTAINER_NAME} is not running"
    exit 1
fi

# Parse command line arguments
FOLLOW=false
LINES=50

while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--follow)
            FOLLOW=true
            shift
            ;;
        -n|--lines)
            LINES="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -f, --follow     Follow log output"
            echo "  -n, --lines N    Show last N lines (default: 50)"
            echo "  -h, --help       Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "📋 Faber Development Logs"
echo "========================="

if [ "$FOLLOW" = true ]; then
    echo "Following logs (press Ctrl+C to stop)..."
    docker exec ${CONTAINER_NAME} tail -f -n ${LINES} /var/log/supervisor/faber.log
else
    echo "Last ${LINES} lines:"
    docker exec ${CONTAINER_NAME} tail -n ${LINES} /var/log/supervisor/faber.log
fi 