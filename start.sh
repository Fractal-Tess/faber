#!/bin/bash

set -e

# Start lldb-server for remote debugging in the background
echo "Starting lldb-server for remote debugging..."
lldb-server platform --server --listen 0.0.0.0:12345 
LLDB_PID=$!
echo "LLDB server started with PID: $LLDB_PID"
