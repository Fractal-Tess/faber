# Faber Project Knowledge Base

**Project**: Secure task execution runtime with container isolation  
**Stack**: Rust (backend) + TypeScript (SDK) + Next.js (docs)  
**Repo**: https://github.com/Fractal-Tess/faber

---

## Overview

Faber runs commands in isolated Linux containers using namespaces and cgroups v2. It provides a REST API for task execution with resource limits and monitoring.

**Key Components:**
- `faber-runtime` — Container isolation, cgroup management, task execution
- `faber-api` — HTTP API server (Axum), routing, caching
- `sdks/js` — TypeScript SDK with TaskBuilder and testing framework
- `docs` — Fumadocs documentation site

---

## Structure

```
.
├── src/                    # Application entry point
├── crates/
│   ├── faber-api/         # HTTP API crate
│   └── faber-runtime/     # Core runtime crate
├── sdks/
│   └── js/                # TypeScript SDK (@faber/runtime-sdk)
├── docs/                  # Documentation site (Next.js + Fumadocs)
├── docker/                # Production & dev Docker configs
├── scripts/               # Integration test scripts
└── research/              # Experiments (cgroup-demo)
```

---

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| API routes | `crates/faber-api/src/handlers/` | Health, execute endpoints |
| Container isolation | `crates/faber-runtime/src/container/` | Namespaces, pivot_root |
| Cgroup management | `crates/faber-runtime/src/cgroup/` | v2 resource limits |
| SDK client | `sdks/js/src/client/` | FaberClient implementation |
| SDK types | `sdks/js/src/types/` | TypeScript definitions |
| SDK tests | `sdks/js/test/` | Unit + integration tests |
| Docs content | `docs/content/docs/` | MDX documentation |

---

## Code Map

**Rust Backend:**

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `Runtime` | struct | `faber-runtime/src/runtime/core.rs` | Main execution engine |
| `Container` | struct | `faber-runtime/src/container/core.rs` | Namespace isolation |
| `CgroupManager` | struct | `faber-runtime/src/cgroup/core.rs` | Resource limits |
| `ExecutionStep` | enum | `faber-runtime/src/task.rs` | Task vs parallel tasks |
| `build_router` | fn | `faber-api/src/router.rs` | Axum route setup |
| `auth_middleware` | fn | `faber-api/src/middleware.rs` | API key validation |

**JavaScript SDK:**

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `FaberClient` | class | `src/client/faber-client.ts` | Main API client |
| `TaskBuilder` | class | `src/builders/task-builder.ts` | Fluent task builder |
| `TestResultAnalyzer` | class | `src/utils/test-result-analyzer.ts` | Test result analysis |
| `TaskTest` | type | `src/types/tests.ts` | Test definition types |

---

## Conventions

### Rust
- **Module structure**: Each crate has `lib.rs` re-exporting public API
- **Builder pattern**: `CgroupConfigBuilder`, `ContainerConfigBuilder`
- **Error handling**: Custom error types in `error.rs`, `Result<T>` aliases
- **Naming**: `snake_case` files, `PascalCase` types, `SCREAMING_SNAKE_CASE` consts

### TypeScript
- **Type aliases preferred** over interfaces (project preference)
- **Naming**: `PascalCase` types, `camelCase` functions/vars
- **Exports**: Explicit named exports, no default exports
- **File organization**: By feature (client/, builders/, types/, utils/)

### Testing
- **Rust**: `cargo test`, integration tests in `tests/` directory
- **SDK**: Vitest, unit tests (`*.test.ts`), integration tests (`*.integration.test.ts`)
- **Integration timeout**: 30 seconds (Docker container lifecycle)

---

## Critical Deployment Rule

**⚠️ MANDATORY: Docker-Only Deployment**

Faber **MUST** run inside a Docker container and **NEVER** directly on the host. Running on the host causes permission errors and cgroup failures.

### Why Docker-Only?
- **Cgroup Permissions**: Faber needs to write to `/sys/fs/cgroup/faber/` which requires container-level isolation
- **Permission Errors**: Running on host produces: `Failed to set memory.max to 'max' in faber cgroup: Permission denied (os error 13)`
- **Namespace Isolation**: Required for proper process and resource isolation

### Correct Deployment
```bash
# Build Docker image with required tools (e.g., GCC for C compilation)
docker build -f docker/prod/Dockerfile -t faber:latest .

# Run with required privileges
docker run -d \
  --privileged \
  --cgroupns=host \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  -e API_KEY=your-api-key \
  -p 3000:3000 \
  faber:latest
```

### Testing C Compilation
To test C compilation, build a custom image with GCC:
```dockerfile
FROM faber:latest

RUN apt-get update && apt-get install -y \
    gcc \
    g++ \
    make \
    libc-dev
```

---

## Anti-Patterns (This Project)

**NEVER:**
- **Run Faber directly on the host** - Always use Docker (see Critical Deployment Rule above)
- Run Docker without `--privileged --cgroupns=host` (Faber requires host cgroup access)
- Use `as any` or `@ts-ignore` in TypeScript (strict types enforced)
- Skip cgroup setup before running tests (causes ENOMEM errors)
- Commit without conventional format (`type: description`)

**ALWAYS:**
- Use type aliases, never interfaces (TypeScript)
- Run `cargo build` before commit (Rust validation)
- Use TaskBuilder for complex task sequences (SDK)
- Clean up cgroup directories after testing (`/sys/fs/cgroup/faber/task-*`)

---

## Commands

```bash
# Rust Backend
cargo build --release    # Production build
cargo test               # Run tests
cargo run                # Dev server (requires cgroup setup)

# JS SDK (cd sdks/js)
npm run build            # Build CJS/ESM/IIFE
npm run dev              # Watch mode
npm run test             # Unit tests
npm run test:integration # Integration tests
npx changeset version    # Version bump + changelog

# Docker
sudo docker run --privileged --cgroupns=host -p 3000:3000 faber
./scripts/test-docker.sh # Full integration test

# Docs (cd docs)
npm run dev              # Next.js dev server
npm run build            # Static site
```

---

## Critical Requirements

### Cgroup Setup (Required Before Running)
```bash
sudo mkdir -p /sys/fs/cgroup/faber
sudo chmod 777 /sys/fs/cgroup/faber
echo "+cpu +memory +pids" | sudo tee /sys/fs/cgroup/faber/cgroup.subtree_control
```

### Docker Requirements
- `--privileged` flag required
- `--cgroupns=host` for cgroup v2 access
- Volume mount: `-v /sys/fs/cgroup:/sys/fs/cgroup:rw`

---

## Release Workflow

1. **JS SDK**: `npx changeset version` → `npm run build` → commit → push
2. **Docker**: Push to main or tag `v*` triggers multi-arch build to Docker Hub
3. **Changesets**: Separate configs for root and SDK (monorepo pattern)

---

## Notes

- **Authentication**: Header-based (`Authorization: Bearer <key>`) or query param
- **Cache**: SHA256-based in-memory caching (optional, `CACHE_ENABLED`)
- **Parallel execution**: Use arrays in task group: `[{cmd:...}, {cmd:...}]`
- **Resource stats**: Memory, CPU, PIDs, execution time tracked per task
- **Exit codes**: 0=success, 1=failure, 127=not found, 128+=signal death

---

**See also:**
- `crates/AGENTS.md` — Rust backend details
- `sdks/js/AGENTS.md` — SDK development patterns
