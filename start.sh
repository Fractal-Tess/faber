#!/bin/sh
set -e

# Start lldb-server for remote debugging in the background
if ! pgrep -f "lldb-server platform" > /dev/null; then
  echo "Starting lldb-server for remote debugging..."
  lldb-server platform --server --listen 0.0.0.0:12345 &
  LLDB_PID=$!
else
  echo "lldb-server already running."
fi

/bin/sh