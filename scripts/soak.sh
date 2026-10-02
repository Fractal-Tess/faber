#!/usr/bin/env bash
# Soak a running Faber with concurrent mixed requests: clean runs, compile
# steps, timeouts, memory hogs, output floods, policy violations, parallel
# steps and clients that hang up. Fails on any response other than 200 or a
# capacity 503, on a wrong outcome, or if slots are still held afterwards.
#
# Usage: API_URL=... API_KEY=... SOAK_SECONDS=120 SOAK_WORKERS=16 scripts/soak.sh

set -Eeuo pipefail

API_URL="${API_URL:-http://localhost:3000/api/v1}"
API_KEY="${API_KEY:-test-key-123}"
SOAK_SECONDS="${SOAK_SECONDS:-120}"
SOAK_WORKERS="${SOAK_WORKERS:-16}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# name | accepted outcomes of the last task | request body. Under this much
# contention the memory hog may run out of wall time before memory.
scenarios=(
    'echo|exited|[{"cmd":"/bin/echo","args":["hi"]}]'
    'compile|exited|[{"cmd":"gcc","args":["m.c","-o","m"],"files":{"m.c":"#include <stdio.h>\nint main(void){puts(\"x\");return 0;}\n"}},{"cmd":"./m"}]'
    'timeout|timed_out|[{"cmd":"/bin/sleep","args":["30"]}]'
    'memory|out_of_memory,timed_out|[{"cmd":"/bin/sh","args":["-c","head -c 1000000000 /dev/zero | tail"]}]'
    'flood|output_limit|[{"cmd":"/bin/sh","args":["-c","yes"]}]'
    'forks|pids_limit|[{"cmd":"/bin/sh","args":["-c","for i in $(seq 1 200); do sleep 2 & done; wait"]}]'
    'policy|policy_violation|[{"cmd":"/bin/sh","args":["-c","exec unshare -m true"]}]'
    'parallel|exited|[[{"cmd":"/bin/echo","args":["a"]},{"cmd":"/bin/echo","args":["b"]},{"cmd":"/bin/echo","args":["c"]}]]'
    'crash|signaled|[{"cmd":"/bin/sh","args":["-c","kill -SEGV $$"]}]'
)

worker() {
    local id=$1 deadline=$2 scenario name expected body status outcome
    while (($(date +%s) < deadline)); do
        scenario="${scenarios[RANDOM % ${#scenarios[@]}]}"
        IFS='|' read -r name expected body <<<"$scenario"
        if ((RANDOM % 10 == 0)); then
            # A client that gives up early; the sandbox must be torn down.
            curl -s -o /dev/null --max-time 0.3 -X POST "$API_URL/execute" \
                -H "Authorization: Bearer $API_KEY" -H 'Content-Type: application/json' -d "$body" || true
            echo "abandoned $name" >>"$work/$id.log"
            continue
        fi
        status="$(curl -s -o "$work/$id.json" -w '%{http_code}' --max-time 60 -X POST "$API_URL/execute" \
            -H "Authorization: Bearer $API_KEY" -H 'Content-Type: application/json' -d "$body" || echo 000)"
        if [[ "$status" == 503 ]]; then
            echo "shed $name" >>"$work/$id.log"
            sleep 0.2
            continue
        fi
        outcome="$(jq -r '[.. | objects | select(has("stats"))] | last | .stats.outcome' "$work/$id.json" 2>/dev/null || true)"
        if [[ "$status" == 200 && -n "$outcome" && ",$expected," == *",$outcome,"* ]]; then
            echo "ok $name" >>"$work/$id.log"
        else
            echo "BAD $name status=$status outcome=$outcome expected=$expected $(head -c 400 "$work/$id.json")" >>"$work/$id.log"
        fi
    done
}

deadline=$(($(date +%s) + SOAK_SECONDS))
for id in $(seq 1 "$SOAK_WORKERS"); do
    worker "$id" "$deadline" &
done
wait

cat "$work"/*.log >"$work/all"
printf 'requests: %s ok, %s shed, %s abandoned, %s bad\n' \
    "$(grep -c '^ok ' "$work/all" || true)" "$(grep -c '^shed ' "$work/all" || true)" \
    "$(grep -c '^abandoned ' "$work/all" || true)" "$(grep -c '^BAD ' "$work/all" || true)"
grep '^ok ' "$work/all" | sort | uniq -c | sed 's/^/  /'
if grep -q '^BAD ' "$work/all"; then
    grep '^BAD ' "$work/all" | sort | uniq -c | sort -rn | head -20
    exit 1
fi

# Everything must drain: abandoned requests hold their slot only until their
# sandbox is gone.
for _ in $(seq 1 30); do
    in_use="$(curl -s "$API_URL/metrics" -H "Authorization: Bearer $API_KEY" | awk '/^faber_execution_slots_in_use /{print $2}')"
    [[ "$in_use" == 0 ]] && break
    sleep 1
done
printf 'slots in use after the run: %s\n' "$in_use"
[[ "$in_use" == 0 ]]
curl -s "$API_URL/health" | grep -q '"status":"ok"'
