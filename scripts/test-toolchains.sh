#!/usr/bin/env bash
# Build the production image with language toolchains, start it, and run
# scripts/toolchain-test.sh against it. Limits are generous: this checks what
# the sandbox profile and filesystem allow, not how the service is tuned.
# FABER_SKIP_IMAGE_BUILD reuses existing images (scripts/vm.sh builds them
# while online). Run it in a disposable VM or on CI, not on a workstation.

set -Eeuo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
PORT="${PORT:-3000}"
API_KEY="${API_KEY:-test-key-123}"

if [[ -z "${FABER_SKIP_IMAGE_BUILD:-}" ]]; then
    docker build -f docker/prod/Dockerfile -t faber-test:latest .
    docker build -f docker/toolchains/Dockerfile -t faber-toolchains:latest docker/toolchains
fi

id="$(docker run -d --privileged --cgroupns=host -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
    -e API_KEY="$API_KEY" -e MEMORY_MAX=1G -e PIDS_MAX=512 -e CPU_MAX=max \
    -e WALL_TIMEOUT_MS=60000 -e CPU_TIME_LIMIT_SECS=60 -e OVERALL_TIMEOUT_MS=120000 \
    -e MAX_CONCURRENCY=2 -e MAX_PARALLEL_TASKS=2 \
    -p "$PORT:3000" faber-toolchains:latest)"
trap 'docker rm -f "$id" >/dev/null' EXIT

API_URL="http://localhost:$PORT/api/v1" API_KEY="$API_KEY" ./scripts/toolchain-test.sh ||
    { docker logs --tail 30 "$id"; exit 1; }
