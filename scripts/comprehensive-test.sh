#!/usr/bin/env bash
# Faber Integration Tests
# Run this against a running Faber container from the host
# Usage: API_URL=http://localhost:3000/api/v1 API_KEY=test-key ./comprehensive-test.sh

set -e

API_URL="${API_URL:-http://localhost:3000/api/v1}"
API_KEY="${API_KEY:-test-key-123}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASSED=0
FAILED=0
WARNINGS=0

run_test() {
    local test_name="$1"
    local curl_cmd="$2"
    local expected_pattern="$3"
    
    echo "  Testing: $test_name"
    response=$(eval "$curl_cmd" 2>&1) || true
    
    if echo "$response" | grep -q "$expected_pattern"; then
        echo -e "    ${GREEN}✓ PASS${NC}"
        PASSED=$((PASSED + 1))
        return 0
    else
        echo -e "    ${RED}✗ FAIL${NC}"
        echo "    Response: $response"
        FAILED=$((FAILED + 1))
        return 1
    fi
}

echo "=== Faber Integration Tests ==="
echo "API URL: $API_URL"
echo "API Key: $API_KEY"
echo ""

# Wait for API to be ready
echo "Waiting for API..."
MAX_RETRIES=30
for i in $(seq 1 $MAX_RETRIES); do
    if curl -s "$API_URL/health" 2>/dev/null | grep -q "ok"; then
        echo -e "${GREEN}✓ API is ready${NC}"
        break
    fi
    [ $i -eq $MAX_RETRIES ] && echo -e "${RED}✗ API unavailable${NC}" && exit 1
    sleep 1
done
echo ""

# Run tests
run_test "Health Check" \
    "curl -s $API_URL/health" \
    "ok"

run_test "Basic Command Execution" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[{\"cmd\":\"/bin/echo\",\"args\":[\"Hello, World!\"]}]'" \
    "Hello, World!"

# Sequential Execution (critical for cgroup cleanup)
run_test "Sequential Execution" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[{\"cmd\":\"/bin/echo\",\"args\":[\"step1\"]},{\"cmd\":\"/bin/echo\",\"args\":[\"step2\"]},{\"cmd\":\"/bin/echo\",\"args\":[\"step3\"]}]'" \
    "step3"

# Parallel Execution
run_test "Parallel Execution" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[[{\"cmd\":\"/bin/echo\",\"args\":[\"p1\"]},{\"cmd\":\"/bin/echo\",\"args\":[\"p2\"]},{\"cmd\":\"/bin/echo\",\"args\":[\"p3\"]}]]'" \
    "p1"

run_test "File Operations" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[{\"cmd\":\"/bin/cat\",\"args\":[\"test.txt\"],\"files\":{\"test.txt\":\"Hello from file!\"}}]'" \
    "Hello from file!"

run_test "Exit Code (success)" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[{\"cmd\":\"/usr/bin/true\"}]'" \
    '\"exit_code\":0'

run_test "Exit Code (failure)" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[{\"cmd\":\"/usr/bin/false\"}]'" \
    '\"exit_code\":1'

run_test "Exit Code (not found)" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[{\"cmd\":\"/nonexistent/command\"}]'" \
    '\"exit_code\":127'

run_test "Resource Stats" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[{\"cmd\":\"/bin/echo\",\"args\":[\"test\"]}]'" \
    "memory_peak_bytes"

# Auth tests
echo "  Testing: Auth - Missing Key"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST $API_URL/execute \
    -H "Content-Type: application/json" \
    -d '[{"cmd":"/bin/echo","args":["test"]}]')
if [ "$HTTP_CODE" = "401" ]; then
    echo -e "    ${GREEN}✓ PASS${NC}"
    PASSED=$((PASSED + 1))
else
    echo -e "    ${RED}✗ FAIL${NC} (got $HTTP_CODE, expected 401)"
    FAILED=$((FAILED + 1))
fi

echo "  Testing: Auth - Wrong Key"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST $API_URL/execute \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer wrong-key" \
    -d '[{"cmd":"/bin/echo","args":["test"]}]')
if [ "$HTTP_CODE" = "401" ]; then
    echo -e "    ${GREEN}✓ PASS${NC}"
    PASSED=$((PASSED + 1))
else
    echo -e "    ${RED}✗ FAIL${NC} (got $HTTP_CODE, expected 401)"
    FAILED=$((FAILED + 1))
fi

run_test "Auth - Correct Key" \
    "curl -s -X POST $API_URL/execute -H 'Content-Type: application/json' -H 'Authorization: Bearer $API_KEY' -d '[{\"cmd\":\"/bin/echo\",\"args\":[\"auth-test\"]}]'" \
    "auth-test"

# Caching test
echo "  Testing: Caching"
RESPONSE1=$(curl -s -X POST $API_URL/execute \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    -d '[{"cmd":"/bin/echo","args":["cache-test-unique-xyz123"]}]')
RESPONSE2=$(curl -s -X POST $API_URL/execute \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    -d '[{"cmd":"/bin/echo","args":["cache-test-unique-xyz123"]}]')
if [ "$RESPONSE1" = "$RESPONSE2" ] && echo "$RESPONSE1" | grep -q "cache-test-unique-xyz123"; then
    echo -e "    ${GREEN}✓ PASS${NC}"
    PASSED=$((PASSED + 1))
else
    echo -e "    ${RED}✗ FAIL${NC}"
    FAILED=$((FAILED + 1))
fi

echo ""
echo "=== Test Summary ==="
echo -e "${GREEN}Passed:${NC} $PASSED"
echo -e "${RED}Failed:${NC} $FAILED"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed${NC}"
    exit 1
fi
