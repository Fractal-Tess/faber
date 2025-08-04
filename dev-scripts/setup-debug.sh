
# Setup script for remote debugging in Docker container
# This script should be run inside the Docker container

set -e

echo "Setting up remote debugging environment..."

# Check if lldb-server is available
if ! command -v lldb-server &> /dev/null; then
    echo "Error: lldb-server not found. Please ensure LLDB is installed in the container."
    exit 1
fi

# Start lldb-server in the background
echo "Starting lldb-server on port 12345..."
lldb-server platform --server --listen *:12345 &

# Store the PID
echo $! > /tmp/lldb-server.pid

echo "lldb-server started with PID $(cat /tmp/lldb-server.pid)"
echo "Remote debugging server is ready on port 12345"
echo ""
echo "You can now:"
echo "1. Use 'Remote Debug (Docker Container)' configuration in VSCode"
echo "2. Or attach to a running process with 'Attach to Running Process (Docker)'"
echo ""
echo "To stop the debug server, run: kill \$(cat /tmp/lldb-server.pid)"

# Keep the script running
wait 