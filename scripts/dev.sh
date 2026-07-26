#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT_DIR/docker/dev/docker-compose.yaml"
CGROUP_ROOT="/sys/fs/cgroup/faber"
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

as_root() {
    if [[ ${EUID} -eq 0 ]]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        printf 'Root access is required to configure %s.\n' "$CGROUP_ROOT" >&2
        exit 1
    fi
}

setup_cgroups() {
    if [[ ! -f /sys/fs/cgroup/cgroup.controllers ]]; then
        printf '%s\n' 'Faber requires a cgroup v2 host.' >&2
        exit 1
    fi

    as_root mkdir -p "$CGROUP_ROOT"
    as_root chmod 0755 "$CGROUP_ROOT"

    local controllers
    controllers="$(< /sys/fs/cgroup/cgroup.controllers)"
    local requested=()
    local controller
    for controller in cpu memory pids; do
        if [[ " $controllers " == *" $controller "* ]]; then
            requested+=("+$controller")
        fi
    done

    if ((${#requested[@]})); then
        as_root sh -c "printf '%s' '${requested[*]}' > '$CGROUP_ROOT/cgroup.subtree_control'"
    fi
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
  up            Configure cgroup v2 and start the hot-reloading dev service
  down          Stop the dev service
  logs          Follow service logs
  shell         Open a shell in the running dev container
  debug         Start gdb against the debug binary in the dev container
  test          Run the Rust test suite in a fresh privileged dev container
  test-security Run focused sandbox isolation and cgroup acceptance tests
  status        Show Compose service status
EOF
}

require_docker

case "${1:-}" in
    up)
        setup_cgroups
        compose up --build --detach
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
    test)
        setup_cgroups
        compose build faber
        compose run --rm faber cargo test --workspace -- --test-threads=1
        ;;
    test-security)
        setup_cgroups
        compose build faber
        compose run --rm faber cargo test -p faber-runtime --test security_acceptance -- --test-threads=1
        ;;
    status)
        compose ps
        ;;
    *)
        usage
        exit 2
        ;;
esac
