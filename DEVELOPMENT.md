# Developing Faber

Faber must run in Docker. Do not run `cargo run`, runtime integration tests, or
the Faber binary directly on the host. The runtime expects a conventional FHS
filesystem and writable cgroup v2 hierarchy; the development image supplies
both without depending on the NixOS host layout.

## Prerequisites

- Rootful Docker with the Compose plugin
- Linux 5.8 or newer with cgroup v2 (`openat2` and procfs `subset=pid` are required and fail closed when unavailable)
- Permission to use Docker

Rootless Docker is not sufficient. Its `privileged` containers cannot acquire
the mount capabilities Faber needs for nested PID and mount namespaces. The
development script detects a rootless user daemon and uses the rootful daemon
through `sudo` when available; otherwise it exits before starting an unsafe or
partially isolated runtime.

## Development loop

Start the development service from the repository root:

```bash
./scripts/dev.sh up
./scripts/dev.sh logs
```

The service runs `cargo watch` inside `debian:bookworm-slim`. Changes under
`src/` or `crates/` trigger a rebuild and restart. Cargo's target, registry, and
git caches live in named Docker volumes, so rebuilding the image does not
discard downloaded dependencies or compiled artifacts.

The API is available at `http://localhost:3000/api/v1` with the development key
`just-a-test-api-key`.

If port 3000 is already in use, select another host port without changing the
port inside the container:

```bash
FABER_PORT=3001 ./scripts/dev.sh up
```

```bash
curl --fail --silent http://localhost:3000/api/v1/health

curl --fail --silent \
  -X POST http://localhost:3000/api/v1/execute \
  -H 'Authorization: Bearer just-a-test-api-key' \
  -H 'Content-Type: application/json' \
  -d '[{"cmd":"/bin/echo","args":["hello"]}]'
```

Stop the service with:

```bash
./scripts/dev.sh down
```

## Tests and debugging

Run formatting and Clippy checks in the development container:

```bash
./scripts/dev.sh check
```

Run Rust tests in a fresh privileged development container rather than on the
host:

```bash
./scripts/dev.sh test
```

Run only the sandbox isolation, cancellation, shutdown, and cgroup acceptance
tests while working on the jailer:

```bash
./scripts/dev.sh test-security
```

Repeat the complete hostile-code suite locally (three rounds by default):

```bash
STRESS_ROUNDS=3 ./scripts/dev.sh test-stress
```

Both commands use a single test thread. This avoids overlapping global cgroup
fixtures and makes lifecycle evidence deterministic; explicit concurrency
stress tests will exercise parallel execution separately. See
[`SECURITY.md`](SECURITY.md) for the threat model, invariant matrix, and the
boundary between local tests and destructive disposable-VM tests. GitHub's
`quality-and-security.yml` runs the complete acceptance suite on a fresh hosted
VM and separately gates Rust, SDK, and documentation quality.

Open a shell or attach GDB to the running debug process:

```bash
./scripts/dev.sh shell
./scripts/dev.sh debug
```

The development container includes GDB, LLDB, procps, and btop. It is
privileged because Faber itself needs namespace, mount, and cgroup operations;
do not expose this container to an untrusted network.

SDK unit tests do not execute Faber and may run in the Nix development shell.
SDK integration tests must target the Docker service:

```bash
cd sdks/js
npm test
FABER_BASE_URL=http://localhost:3000 \
  FABER_API_KEY=just-a-test-api-key \
  npm run test:integration
```

## Running in a disposable VM

The development container is privileged and sees the host cgroup tree, so it is
root-equivalent on the machine that runs it. Docker keeps Faber off the host
filesystem layout; it does not protect the host from a sandbox regression, a
fork bomb, or a cgroup mistake. `scripts/vm.sh` runs the same workflow inside a
KVM guest (NixOS, defined in `nix/test-vm.nix`) so those stop at the VM
boundary. It needs Nix and `/dev/kvm`, and no sudo or host Docker.

```bash
./scripts/vm.sh prepare        # once, and after dependency or Dockerfile changes
./scripts/vm.sh test-security  # also: check, test, test-stress
./scripts/vm.sh test-docker    # production image smoke test
./scripts/vm.sh up             # dev API on 127.0.0.1:$FABER_PORT, Ctrl-C to stop
./scripts/vm.sh demo           # demo backend on 127.0.0.1:${FABER_PORT:-3300}
./scripts/vm.sh exec probe.sh  # run a host script as root inside the guest
./scripts/vm.sh shell          # root console; poweroff to leave
./scripts/vm.sh reset          # delete the guest disk and caches
```

`prepare` is the only step with network access: it builds the dev and
production images and compiles the tests, and runs no Faber code. Everything else boots with guest
networking cut off (`--online` opts back in), so code under test cannot reach
the host's loopback services or the LAN. The repository is exported read-only;
the guest disk with the Docker image and Cargo caches lives in
`~/.cache/faber-vm`. The guest serves a snapshot: `up` does not hot-reload on
host edits, so restart it to pick up changes.

## Production smoke test

Set an API key explicitly, then start the production Compose service:

```bash
sudo env \
  API_KEY="$(openssl rand -hex 32)" \
  FABER_BIND_ADDRESS=127.0.0.1 \
  FABER_PORT=3001 \
  docker compose -f docker/prod/docker-compose.yaml up --build --detach
```

Production and development Compose configurations both use the host cgroup
namespace and mount `/sys/fs/cgroup` read-write, which the nested-runtime
design requires. Both also cap the container's memory and process count above
Faber's own aggregate limits, as a backstop for the host should those limits
fail. They are not a substitute for the inner sandbox controls described in
the project roadmap.
