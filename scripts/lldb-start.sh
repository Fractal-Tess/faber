#!/bin/bash
set -e
# Start lldb-server for remote debugging in the background
echo "Starting lldb-server for remote debugging..."
lldb-server platform --server --listen *:12345 &
LLDB_PID=$!

# Wait for lldb-server to exit
wait $LLDB_PID
