#!/bin/bash

echo "🧪 Testing Faber Resource Tracking"
echo "=================================="

echo "⏳ Waiting 10 seconds for server to start..."
sleep 10

echo "🚀 Sending test request to Faber API..."

curl -X POST http://localhost:3000/execute-tasks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer open-mode-no-auth" \
  -d '[
    {
        "command": "/bin/echo",
        "args":[ "hello world" ]
    },
    {
        "command":"g++",
        "args":[
            "main.cpp",
            "-o",
            "program"
        ],
        "files":{
            "main.cpp":"#include <iostream>\n#include <vector>\n\nint main() {\n    std::vector<int> numbers = {1, 2, 3, 4, 5};\n    \n    std::cout << \"Numbers: \";\n    for (const auto& num : numbers) {\n        std::cout << num << \" \";\n    }\n    std::cout << std::endl;\n    \n    return 0;\n}"
        }
    },
    {
        "command":"./program"
    }
]' | jq '.'

echo ""
echo "✅ Test completed!" 