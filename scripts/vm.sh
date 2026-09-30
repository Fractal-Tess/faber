#!/usr/bin/env bash
# Run Faber's privileged Docker workflow inside a disposable KVM guest.
#
# scripts/dev.sh starts a privileged container with the host cgroup tree,
# which is root-equivalent on the machine it runs on. This wrapper boots the
# NixOS guest from nix/test-vm.nix and runs dev.sh there instead. QEMU runs as
# the calling user; nothing here needs sudo or the host Docker daemon.

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${FABER_VM_STATE:-${XDG_CACHE_HOME:-$HOME/.cache}/faber-vm}"
RUN_DIR="$STATE_DIR/last-run"
SHARED_DIR="$RUN_DIR/shared"
DISK_IMAGE="$STATE_DIR/disk.qcow2"
HOST_PORT="${FABER_PORT:-3000}"
DEMO_PORT="${FABER_PORT:-3300}"
COMPOSE='docker compose -f docker/dev/docker-compose.yaml'

usage() {
    cat <<'EOF'
Usage: scripts/vm.sh <command> [--online]

Commands:
  prepare        Build the dev and production images and compile the tests
                 (needs network; runs no Faber code)
  check          scripts/dev.sh check inside the guest
  test           scripts/dev.sh test inside the guest
  test-security  scripts/dev.sh test-security inside the guest
  test-stress    scripts/dev.sh test-stress inside the guest (STRESS_ROUNDS)
  test-docker    scripts/test-docker.sh against the production image
  up             Serve the dev API on 127.0.0.1:$FABER_PORT until interrupted
  demo           Serve demo/compose.yaml (production image) on
                 127.0.0.1:${FABER_PORT:-3300} until interrupted
  exec <script>  Run a host script as root inside the guest, from /faber-src
  shell          Root console in the guest (poweroff to leave)
  reset          Delete the guest disk and its caches

Everything except prepare runs with guest networking cut off; pass --online
to allow outbound access. The repository is mounted read-only at /faber-src.

Environment:
  FABER_VM_MEMORY_MB  Guest memory (default 8192)
  FABER_VM_CPUS       Guest CPUs (default: host CPUs, at most 8)
  FABER_VM_TIMEOUT    Seconds before the guest is killed (default 3600;
                      up and shell have no limit)
  FABER_VM_STATE      Disk and run directory (default ~/.cache/faber-vm)
EOF
}

die() {
    printf '%s\n' "$*" >&2
    exit 1
}

default_cpus() {
    local cpus
    cpus="$(nproc)"
    ((cpus > 8)) && cpus=8
    printf '%s' "$cpus"
}

build_vm() {
    command -v nix >/dev/null 2>&1 || die 'Nix is required to build the guest.'
    [[ -r /dev/kvm && -w /dev/kvm ]] || die '/dev/kvm is not accessible; the guest needs KVM.'
    nix build \
        --extra-experimental-features 'nix-command flakes' \
        --no-link --print-out-paths \
        --file "$ROOT_DIR/nix/test-vm.nix"
}

# vm_env <network: online|offline|forward> <command...>
# Replaces this shell with a command in the environment the guest runner reads.
vm_env() {
    local network=$1 net_opts=''
    case "$network" in
        online) ;;
        offline) net_opts='restrict=on' ;;
        forward) net_opts="restrict=on,hostfwd=tcp:127.0.0.1:${HOST_PORT}-:3000" ;;
        forward-demo) net_opts="restrict=on,hostfwd=tcp:127.0.0.1:${DEMO_PORT}-:3300" ;;
    esac

    # The pinned QEMU must not pick up libraries from the caller's session.
    exec env --unset=LD_LIBRARY_PATH --unset=LD_PRELOAD \
        FABER_VM_SRC="$ROOT_DIR" \
        SHARED_DIR="$SHARED_DIR" \
        NIX_DISK_IMAGE="$DISK_IMAGE" \
        QEMU_NET_OPTS="$net_opts" \
        QEMU_OPTS="-m ${FABER_VM_MEMORY_MB:-8192} -smp ${FABER_VM_CPUS:-$(default_cpus)}" \
        "${@:2}"
}

# run_job <network> <timeout>: boot the guest, run $SHARED_DIR/job.sh in it,
# stream its output and exit with its status.
run_job() {
    local network=$1 limit=$2
    : >"$SHARED_DIR/output.log"

    # The guest gets its own session, so an interrupt reaches this script
    # only and the guest can be asked to stop instead of being killed.
    local launcher=(setsid)
    ((limit > 0)) && launcher+=(timeout --kill-after=10 "$limit")
    (vm_env "$network" "${launcher[@]}" "$VM_RUNNER") \
        </dev/null >"$RUN_DIR/console.log" 2>&1 &
    local vm_pid=$!

    # First interrupt asks the guest to power off; it is killed if it has not
    # done so within a minute, or at once on a second interrupt.
    local stopping=0 killer_pid=''
    stop_guest() {
        if ((stopping)); then
            kill -KILL -- "-$vm_pid" 2>/dev/null || true
            return
        fi
        stopping=1
        printf '\nStopping the guest...\n' >&2
        touch "$SHARED_DIR/stop"
        (sleep 60 && kill -KILL -- "-$vm_pid" 2>/dev/null) 9>&- &
        killer_pid=$!
    }
    trap stop_guest INT TERM

    tail --pid="$vm_pid" -n +1 -F "$SHARED_DIR/output.log" 2>/dev/null &
    local tail_pid=$!
    while kill -0 "$vm_pid" 2>/dev/null; do
        wait "$vm_pid" 2>/dev/null || true
    done
    wait "$tail_pid" 2>/dev/null || true
    trap - INT TERM
    [[ -z "$killer_pid" ]] || kill "$killer_pid" 2>/dev/null || true

    if [[ -s "$SHARED_DIR/exit-code" ]]; then
        return "$(<"$SHARED_DIR/exit-code")"
    fi
    ((stopping)) && return 130
    printf 'The guest stopped without reporting a result. Console log: %s\n' \
        "$RUN_DIR/console.log" >&2
    return 1
}

command="${1:-}"
[[ -n "$command" ]] || { usage; exit 2; }
shift

network=offline
args=()
for arg in "$@"; do
    case "$arg" in
        --online) network=online ;;
        *) args+=("$arg") ;;
    esac
done

case "$command" in
    -h | --help | help)
        usage
        exit 0
        ;;
    reset)
        rm -rf "$STATE_DIR"
        printf 'Removed %s\n' "$STATE_DIR"
        exit 0
        ;;
    prepare | check | test | test-security | test-stress | test-docker | up | demo | exec | shell) ;;
    *)
        usage
        exit 2
        ;;
esac

mkdir -p "$STATE_DIR"
exec 9>"$STATE_DIR/lock"
flock --nonblock 9 || die "Another guest is already using $STATE_DIR."
rm -rf "$RUN_DIR"
mkdir -p "$SHARED_DIR"

limit="${FABER_VM_TIMEOUT:-3600}"
# Without network the image built by `prepare` is reused as it is.
offline_env=''
[[ "$network" == online ]] || offline_env='export FABER_SKIP_IMAGE_BUILD=1'
job="$SHARED_DIR/job.sh"
vm="$(build_vm)"
VM_RUNNER="$(echo "$vm"/bin/run-*-vm)"

case "$command" in
    prepare)
        cat >"$job" <<EOF
set -Eeuo pipefail
$COMPOSE build faber
$COMPOSE run --rm --no-TTY faber cargo test --workspace --no-run
docker build -f docker/prod/Dockerfile -t faber-test:latest .
docker tag faber-test:latest vgfractal/faber:latest
EOF
        run_job online "$limit"
        ;;
    check | test | test-security | test-stress)
        cat >"$job" <<EOF
export STRESS_ROUNDS=${STRESS_ROUNDS:-3}
$offline_env
exec ./scripts/dev.sh $command
EOF
        run_job "$network" "$limit"
        ;;
    test-docker)
        cat >"$job" <<EOF
$offline_env
exec ./scripts/test-docker.sh
EOF
        run_job "$network" "$limit"
        ;;
    demo)
        [[ "$network" == offline ]] && network=forward-demo
        cat >"$job" <<EOF
export FABER_BIND_ADDRESS=0.0.0.0 FABER_PORT=3300
docker compose -f demo/compose.yaml up --detach
exec docker compose -f demo/compose.yaml logs --follow faber
EOF
        printf 'Forwarding http://127.0.0.1:%s/api/v1 to the demo in the guest. Interrupt to stop.\n' "$DEMO_PORT"
        run_job "$network" 0
        ;;
    up)
        [[ "$network" == offline ]] && network=forward
        cat >"$job" <<EOF
export FABER_BIND_ADDRESS=0.0.0.0
$offline_env
./scripts/dev.sh up || echo 'Faber is still starting; following its logs.'
exec $COMPOSE logs --follow faber
EOF
        printf 'Forwarding http://127.0.0.1:%s/api/v1 to the guest. Interrupt to stop.\n' "$HOST_PORT"
        run_job "$network" 0
        ;;
    exec)
        [[ ${#args[@]} -eq 1 && -f "${args[0]}" ]] || die 'Usage: scripts/vm.sh exec <script> [--online]'
        { printf '%s\n' "$offline_env"; cat "${args[0]}"; } >"$job"
        run_job "$network" "$limit"
        ;;
    shell)
        vm_env "$network" "$VM_RUNNER"
        ;;
esac
