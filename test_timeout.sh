#!/bin/bash

echo "Testing Faber timeout functionality..."
echo "Current timeout setting: 10 seconds"
echo "Sending a task that sleeps for 60 seconds..."

# Test the timeout with a long-running task
curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '[
    {
      "cmd": "/bin/sleep",
      "args": ["60"]
    }
  ]' \
  -w "\nHTTP Status: %{http_code}\nResponse Time: %{time_total}s\n"

echo ""
echo "If timeout is working, you should see:"
echo "- Task killed after ~10 seconds"
echo "- ProcessManagement error with timeout details"
echo "- HTTP status indicating an error"
