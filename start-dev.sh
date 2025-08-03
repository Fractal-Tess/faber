#!/bin/bash

# Development container startup script
echo "Starting Faber development container..."

# Check if binary exists
if [ ! -f /opt/faber ]; then
    echo "ERROR: Binary not found at /opt/faber"
    echo "Make sure to mount the binary when running the container"
    exit 1
fi

# Check if binary is executable
if [ ! -x /opt/faber ]; then
    echo "ERROR: Binary is not executable"
    chmod +x /opt/faber
fi

echo "Binary found and executable: $(ls -la /opt/faber)"
echo "Starting supervisor..."

# Start supervisor
exec /usr/bin/supervisord -c /etc/supervisor/supervisord.conf 