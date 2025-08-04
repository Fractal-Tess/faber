#!/bin/bash

echo "🧪 Testing Resource Limits with Cgroups"
echo "======================================="

echo "🚀 Sending test request with resource-intensive tasks..."

curl -X POST http://localhost:3000/execute-tasks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer open-mode-no-auth" \
  -d '[
    {
        "command": "python3",
        "args": ["-c", "import time; time.sleep(30)"],
        "files": {
            "timeout_test.py": "import time\ntime.sleep(30)"
        }
    },
    {
        "command": "python3",
        "args": ["-c", "import os; os.system(\"dd if=/dev/zero of=/tmp/test bs=1M count=1000\")"],
        "files": {
            "memory_test.py": "import os\nimport time\n\n# Try to allocate a lot of memory\nmemory = []\nfor i in range(1000000):\n    memory.append(\"x\" * 1000000)\n    if i % 100 == 0:\n        print(f\"Allocated {i} MB\")\n"
        }
    },
    {
        "command": "python3",
        "args": ["-c", "import math; [math.factorial(i) for i in range(100000)]"],
        "files": {
            "cpu_test.py": "import math\n\n# CPU intensive task\nfor i in range(1000000):\n    result = math.factorial(1000)\n    if i % 1000 == 0:\n        print(f\"Computed factorial {i} times\")\n"
        }
    }
]' | jq '.'

echo ""
echo "✅ Resource limit test completed!" 