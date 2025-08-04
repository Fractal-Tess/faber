#!/bin/bash

echo "🧪 Comprehensive Resource Limit Testing"
echo "======================================"

echo ""
echo "1️⃣ Testing Wall Time Limit (should timeout after 30s)..."
curl -X POST http://localhost:3000/execute-tasks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer open-mode-no-auth" \
  -d '[{"command":"python3","args":["-c","import time; time.sleep(35)"],"files":{"timeout_test.py":"import time\ntime.sleep(35)"}}]' | jq '.results[0].status, .results[0].resource_limits_exceeded'

echo ""
echo "2️⃣ Testing Normal Task (should succeed)..."
curl -X POST http://localhost:3000/execute-tasks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer open-mode-no-auth" \
  -d '[{"command":"echo","args":["Hello World"]}]' | jq '.results[0].status, .results[0].resource_usage.wall_time_ns'

echo ""
echo "3️⃣ Testing Multiple Tasks with Mixed Results..."
curl -X POST http://localhost:3000/execute-tasks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer open-mode-no-auth" \
  -d '[
    {"command":"echo","args":["Task 1 - Success"]},
    {"command":"python3","args":["-c","import time; time.sleep(35)"],"files":{"timeout.py":"import time\ntime.sleep(35)"}},
    {"command":"echo","args":["Task 3 - Should be skipped"]}
  ]' | jq '.results[] | {task: .stdout, status: .status, wall_time_exceeded: .resource_limits_exceeded.wall_time_limit_exceeded}'

echo ""
echo "✅ Comprehensive test completed!" 