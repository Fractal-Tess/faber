#!/usr/bin/env bash
# Compile and run a small program in each common language through a running
# Faber built from docker/toolchains/Dockerfile, under the default sandbox
# profile. Catches syscalls and files a real toolchain needs that the sandbox
# refuses.
#
# Usage: API_URL=http://localhost:3000/api/v1 API_KEY=... scripts/toolchain-test.sh

set -Eeuo pipefail

API_URL="${API_URL:-http://localhost:3000/api/v1}"
API_KEY="${API_KEY:-test-key-123}"
PASSED=0
FAILED=0

# check <name> <expected stdout of the last step> <request body>
check() {
    local name=$1 expected=$2 body=$3 response last
    response="$(curl -sS --max-time 120 -X POST "$API_URL/execute" \
        -H "Authorization: Bearer $API_KEY" -H 'Content-Type: application/json' -d "$body")" || true
    last="$(jq -r '[.. | objects | select(has("stats"))] | last | "\(.exit_code) \(.stats.outcome) \(.stdout)"' \
        <<<"$response" 2>/dev/null || true)"
    if [[ "$last" == "0 exited $expected" ]]; then
        printf '  ok    %s\n' "$name"
        PASSED=$((PASSED + 1))
    else
        printf '  FAIL  %s\n' "$name"
        jq -c '.. | objects | select(has("stats")) | {exit_code, outcome: .stats.outcome, stdout, stderr: (.stderr // .error)}' \
            <<<"$response" 2>/dev/null | cut -c1-1200 || printf '%s\n' "$response" | cut -c1-1200
        FAILED=$((FAILED + 1))
    fi
}

for _ in $(seq 1 60); do
    curl -fsS "$API_URL/health" >/dev/null 2>&1 && break
    sleep 1
done

TOOL_ENV='{"HOME":"/tmp","GOCACHE":"/tmp/go-cache","GOFLAGS":"-buildvcs=false"}'

check 'shell and coreutils' 'faber' "$(jq -n '[
    {cmd:"/bin/sh", args:["-c","printf faber | tr a-z A-Z | tr A-Z a-z; id -un >/dev/null; ls / >/dev/null; date >/dev/null"]}]')"

check 'C (gcc)' 'hello c' "$(jq -n '[
    {cmd:"gcc", args:["main.c","-O2","-o","main"], files:{"main.c":"#include <stdio.h>\nint main(void){printf(\"hello c\");return 0;}\n"}},
    {cmd:"./main"}]')"

check 'C++ (g++, threads)' 'hello c++ 4' "$(jq -n '[
    {cmd:"g++", args:["main.cpp","-O2","-pthread","-o","main"], files:{"main.cpp":"#include <atomic>\n#include <iostream>\n#include <thread>\n#include <vector>\nint main(){std::atomic<int> n{0};std::vector<std::thread> t;for(int i=0;i<4;i++)t.emplace_back([&]{n++;});for(auto&x:t)x.join();std::cout<<\"hello c++ \"<<n;}\n"}},
    {cmd:"./main"}]')"

check 'make' 'built' "$(jq -n '[
    {cmd:"make", args:["-s"], files:{"Makefile":"all:\n\t@gcc x.c -o x && ./x\n","x.c":"#include <stdio.h>\nint main(void){printf(\"built\");return 0;}\n"}}]')"

check 'Python' 'hello python 5050' "$(jq -n '[
    {cmd:"python3", args:["main.py"], files:{"main.py":"import json, os, subprocess, tempfile, threading\nt = threading.Thread(target=lambda: None); t.start(); t.join()\nwith tempfile.NamedTemporaryFile() as f: f.write(b\"x\")\nsubprocess.run([\"true\"], check=True)\nprint(\"hello python\", sum(range(101)), end=\"\")\n"}}]')"

check 'Node.js' 'hello node 3' "$(jq -n '[
    {cmd:"node", args:["main.js"], files:{"main.js":"const fs = require(\"fs\"); const os = require(\"os\"); const { execSync } = require(\"child_process\");\nfs.writeFileSync(\"x.txt\", \"abc\"); execSync(\"true\");\nsetTimeout(() => process.stdout.write(\"hello node \" + fs.readFileSync(\"x.txt\").length), 10);\n"}}]')"

check 'Java (javac, java)' 'hello java 4' "$(jq -n --argjson env "$TOOL_ENV" '[
    {cmd:"javac", args:["Main.java"], env:$env, files:{"Main.java":"import java.util.concurrent.*;\npublic class Main { public static void main(String[] a) throws Exception { ExecutorService e = Executors.newFixedThreadPool(4); Callable<Integer> one = () -> 1; int n = 0; for (Future<Integer> f : e.invokeAll(java.util.Collections.nCopies(4, one))) n += f.get(); e.shutdown(); System.out.print(\"hello java \" + n); } }\n"}},
    {cmd:"java", args:["-Xmx64m","Main"], env:$env}]')"

check 'Go' 'hello go 4' "$(jq -n --argjson env "$TOOL_ENV" '[
    {cmd:"go", args:["build","-o","main","main.go"], env:$env, files:{"main.go":"package main\n\nimport (\n\t\"fmt\"\n\t\"sync\"\n)\n\nfunc main() {\n\tvar wg sync.WaitGroup\n\tvar mu sync.Mutex\n\tn := 0\n\tfor i := 0; i < 4; i++ {\n\t\twg.Add(1)\n\t\tgo func() { defer wg.Done(); mu.Lock(); n++; mu.Unlock() }()\n\t}\n\twg.Wait()\n\tfmt.Print(\"hello go \", n)\n}\n"}},
    {cmd:"./main"}]')"

check 'Rust (rustc)' 'hello rust 4' "$(jq -n --argjson env "$TOOL_ENV" '[
    {cmd:"rustc", args:["-O","main.rs"], env:$env, files:{"main.rs":"use std::thread;\nfn main() { let n: i32 = (0..4).map(|_| thread::spawn(|| 1)).map(|h| h.join().unwrap()).sum(); print!(\"hello rust {}\", n); }\n"}},
    {cmd:"./main"}]')"

check 'native profile runs a compiled binary' 'hello c' "$(jq -n '[
    {cmd:"gcc", args:["main.c","-o","main"], files:{"main.c":"#include <stdio.h>\nint main(void){printf(\"hello c\");return 0;}\n"}},
    {cmd:"./main", sandbox_profile:"native_v2"}]')"

printf '\n%d passed, %d failed\n' "$PASSED" "$FAILED"
[[ "$FAILED" -eq 0 ]]
