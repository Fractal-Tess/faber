#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT_DIR/docker/dev/docker-compose.yaml"
HOST_PORT="${FABER_PORT:-3000}"
DOCKER=(docker)

compose() {
    "${DOCKER[@]}" compose -f "$COMPOSE_FILE" "$@"
}

require_docker() {
    local security_options
    if security_options="$(docker info --format '{{json .SecurityOptions}}' 2>/dev/null)"; then
        if [[ "$security_options" == *'name=rootless'* ]]; then
            if sudo -n docker info >/dev/null 2>&1; then
                DOCKER=(sudo -n --preserve-env=FABER_PORT docker)
            else
                printf '%s\n' 'Faber requires rootful Docker; the current daemon is rootless and no rootful daemon is available through sudo.' >&2
                exit 1
            fi
        fi
    elif sudo -n docker info >/dev/null 2>&1; then
        DOCKER=(sudo -n --preserve-env=FABER_PORT docker)
    else
        printf '%s\n' 'Docker is unavailable. Start a rootful Docker daemon.' >&2
        exit 1
    fi

    if ! "${DOCKER[@]}" compose version >/dev/null 2>&1; then
        printf '%s\n' 'The Docker Compose plugin is required.' >&2
        exit 1
    fi
}

# Faber builds its cgroup hierarchy inside the container's own cgroup, so
# nothing has to be created on the host beforehand.
require_cgroup_v2() {
    if [[ ! -f /sys/fs/cgroup/cgroup.controllers ]]; then
        printf '%s\n' 'Faber requires a cgroup v2 host.' >&2
        exit 1
    fi
}

# FABER_SKIP_IMAGE_BUILD reuses the existing image. scripts/vm.sh sets it for
# offline guests, where the base image cannot be resolved from the registry.
build_image() {
    [[ -n "${FABER_SKIP_IMAGE_BUILD:-}" ]] || compose build faber
}

wait_until_healthy() {
    local attempts=60
    while ((attempts > 0)); do
        if [[ "$(compose ps --format json faber 2>/dev/null || true)" == *'"Health":"healthy"'* ]]; then
            return 0
        fi
        sleep 1
        ((attempts--))
    done

    compose logs faber
    printf '%s\n' 'Faber did not become healthy within 60 seconds.' >&2
    return 1
}

usage() {
    cat <<'EOF'
Usage: scripts/dev.sh <command>

Commands:
  up            Start the hot-reloading dev service
  down          Stop the dev service
  logs          Follow service logs
  shell         Open a shell in the running dev container
  debug         Start gdb against the debug binary in the dev container
  check         Run Rust formatting and Clippy checks in the dev container
  test          Run the Rust test suite in a fresh privileged dev container
  test-security Run focused sandbox isolation and cgroup acceptance tests
  test-stress   Repeat the complete adversarial suite (STRESS_ROUNDS, default 3)
  status        Show Compose service status
EOF
}

require_docker

case "${1:-}" in
    up)
        require_cgroup_v2
        build_image
        compose up --detach
        wait_until_healthy
        printf 'Faber is healthy at http://localhost:%s/api/v1/health\n' "$HOST_PORT"
        ;;
    down)
        compose down --remove-orphans
        ;;
    logs)
        compose logs --follow faber
        ;;
    shell)
        compose exec faber bash
        ;;
    debug)
        compose exec faber bash -lc 'pid="$(pgrep -n -x faber)"; exec gdb -p "$pid"'
        ;;
    check)
        build_image
        compose run --rm --no-TTY faber cargo fmt --all -- --check
        compose run --rm --no-TTY faber cargo clippy --workspace --all-targets -- \
            -D warnings \
            -A clippy::collapsible-if \
            -A clippy::io-other-error \
            -A clippy::len-without-is-empty \
            -A clippy::ptr-arg \
            -A clippy::suspicious-open-options \
            -A clippy::trim-split-whitespace
        ;;
    test)
        require_cgroup_v2
        build_image
        compose run --rm --no-TTY faber cargo test --workspace --no-fail-fast -- --test-threads=1
        ;;
    test-security)
        require_cgroup_v2
        build_image
        compose run --rm --no-TTY faber bash -lc \
            'cargo test -p faber-runtime --test security_acceptance -- --test-threads=1 && cargo test -p faber-runtime --test shutdown && cargo test -p faber-api --test cancellation -- --test-threads=1'
        ;;
    test-stress)
        require_cgroup_v2
        build_image
        compose run --rm --no-TTY -e STRESS_ROUNDS="${STRESS_ROUNDS:-3}" faber bash -lc \
            'set -Eeuo pipefail; for round in $(seq 1 "$STRESS_ROUNDS"); do echo "=== adversarial round $round/$STRESS_ROUNDS ==="; cargo test -p faber-runtime --test security_acceptance -- --test-threads=1; cargo test -p faber-runtime --test shutdown; cargo test -p faber-api --test cancellation -- --test-threads=1; done'
        ;;
    status)
        compose ps
        ;;
    *)
        usage
        exit 2
        ;;
esac
