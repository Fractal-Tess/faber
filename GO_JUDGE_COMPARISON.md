# go-judge vs Faber: Feature Comparison

This document analyzes what go-judge does differently from Faber, identifying features that could be implemented to reach feature parity.

## Executive Summary

go-judge is a more mature project with additional features around file management, streaming, multiple transport protocols, and cross-platform support. Faber has a solid foundation but lacks several key features for production online judge use cases.

---

## Feature Comparison Matrix

| Feature | go-judge | Faber | Priority |
|---------|----------|-------|----------|
| **Core Execution** |
| Command execution | ✅ | ✅ | - |
| Sequential tasks | ✅ | ✅ | - |
| Parallel tasks | ✅ | ✅ | - |
| Resource limits (CPU, memory, PIDs) | ✅ | ✅ | - |
| Timeout handling | ✅ | ✅ | - |
| **File Management** |
| Input files (copyIn) | ✅ | ✅ (`files` field) | - |
| Output files (copyOut) | ✅ | ❌ | High |
| Cached file store | ✅ | ❌ | High |
| File persistence between requests | ✅ | ❌ | High |
| Symlink support | ✅ | ❌ | Low |
| **I/O Handling** |
| stdin input | ✅ | ✅ | - |
| stdout/stderr capture | ✅ | ✅ | - |
| Pipe mapping between processes | ✅ | ❌ | Medium |
| TTY support | ✅ | ❌ | Low |
| **Transport Protocols** |
| REST API | ✅ | ✅ | - |
| gRPC | ✅ | ❌ | Medium |
| WebSocket | ✅ | ❌ | Medium |
| WebSocket streaming | ✅ | ❌ | Medium |
| FFI (Foreign Function Interface) | ✅ | ❌ | Low |
| **Resource Control** |
| CPU limit (time) | ✅ | ✅ | - |
| Clock/wall time limit | ✅ | ✅ | - |
| Memory limit | ✅ | ✅ | - |
| Process limit | ✅ | ✅ | - |
| CPU rate limiting | ✅ | ❌ | Medium |
| CPU set (affinity) | ✅ | ❌ | Low |
| Stack limit | ✅ | ❌ | Low |
| Output size limit | ✅ | ❌ | Medium |
| **Security** |
| Namespace isolation | ✅ | ✅ | - |
| Cgroup limits | ✅ | ✅ | - |
| Seccomp filtering | ✅ (optional) | ❌ | Medium |
| Capability dropping | ✅ | ✅ | - |
| User namespace | ✅ | ❌ | Medium |
| **Monitoring** |
| Prometheus metrics | ✅ | ❌ | Medium |
| Debug endpoints | ✅ | ❌ | Low |
| Version/config endpoints | ✅ | ❌ | Low |
| **Platform Support** |
| Linux | ✅ | ✅ | - |
| Windows (experimental) | ✅ | ❌ | Low |
| macOS (experimental) | ✅ | ❌ | Low |
| **Cgroup Support** |
| Cgroup v1 | ✅ | ❌ | Low |
| Cgroup v2 | ✅ | ✅ | - |
| systemd integration | ✅ | ❌ | Low |
| clone3(CLONE_INTO_CGROUP) | ✅ | ❌ | Low |

---

## Detailed Feature Analysis

### 1. File Store System (High Priority)

**go-judge implementation:**
```
POST /file         - Upload file to in-memory store, returns fileId
GET /file          - List all cached files
GET /file/:fileId  - Download file content
DELETE /file/:fileId - Delete cached file
```

**How it works:**
- Files are stored in shared memory (`/dev/shm/`) by default
- Each file gets a unique `fileId`
- Files can be referenced in subsequent `/run` requests via `fileId`
- Files have TTL (configurable via `-file-timeout`)
- Enables compile-once, run-many pattern

**Example workflow:**
```json
// Step 1: Compile and cache the binary
{
  "cmd": [{
    "args": ["/usr/bin/g++", "a.cc", "-o", "a"],
    "copyIn": { "a.cc": { "content": "..." } },
    "copyOutCached": ["a"]  // Cache the compiled binary
  }]
}
// Response: { "fileIds": { "a": "5LWIZAA45JHX4Y4Z" } }

// Step 2: Run with cached binary (multiple test cases)
{
  "cmd": [{
    "args": ["a"],
    "copyIn": { "a": { "fileId": "5LWIZAA45JHX4Y4Z" } }
  }]
}
```

**Faber gap:**
- No file persistence between requests
- No file store API
- Must include source code in every request

**Implementation suggestions:**
- Add `FileStore` service with in-memory HashMap
- Add `/api/v1/file` endpoints
- Add `copyOut` and `copyOutCached` fields to Task
- Add `fileId` reference type for `files` field

---

### 2. CopyOut - Output File Collection (High Priority)

**go-judge implementation:**
```typescript
interface Cmd {
  copyOut?: string[];        // Files to return in response
  copyOutCached?: string[];  // Files to cache and return fileId
  copyOutMax?: number;       // Max size limit
  copyOutDir?: string;       // Dump entire directory
}
```

**Example:**
```json
{
  "cmd": [{
    "args": ["./compile.sh"],
    "copyOut": ["stdout", "stderr", "output.txt"],
    "copyOutCached": ["binary"],
    "copyOutMax": 10485760
  }]
}
```

**Response:**
```json
{
  "files": { "stdout": "...", "stderr": "...", "output.txt": "..." },
  "fileIds": { "binary": "ABCD1234" }
}
```

**Faber gap:**
- Can only get stdout/stderr
- Cannot extract files created during execution

---

### 3. Pipe Mapping (Medium Priority)

**go-judge implementation:**
```typescript
interface PipeMap {
  in: { index: number; fd: number };   // Source process and fd
  out: { index: number; fd: number };  // Destination process and fd
  proxy?: boolean;  // Enable content capture
  name?: string;    // Name for captured content
  max?: number;     // Max capture size
}
```

**Use case:** Interactive problems where two programs communicate:
```json
{
  "cmd": [
    { "args": ["./judge"], "files": [null, null, {...}] },
    { "args": ["./solution"], "files": [null, null, {...}] }
  ],
  "pipeMapping": [
    { "in": {"index": 0, "fd": 1}, "out": {"index": 1, "fd": 0} },
    { "in": {"index": 1, "fd": 1}, "out": {"index": 0, "fd": 0} }
  ]
}
```

**Faber gap:**
- No inter-process communication support
- Cannot pipe output of one task to input of another

---

### 4. WebSocket Streaming (Medium Priority)

**go-judge implementation:**
- `/ws` - WebSocket version of `/run`
- `/stream` - Interactive streaming with live I/O

**Stream protocol:**
```
Request types:
  1 = request (JSON encoded)
  2 = resize (terminal resize)
  3 = input (stdin data)
  4 = cancel

Response types:
  1 = response (JSON result)
  2 = output (stdout/stderr data)
```

**Use cases:**
- Real-time output streaming
- Interactive terminal sessions
- Long-running process monitoring

**Faber gap:**
- Only synchronous HTTP
- No real-time output streaming
- No interactive sessions

---

### 5. gRPC Support (Medium Priority)

**go-judge implementation:**
- Full gRPC API parallel to REST
- Protobuf definitions in `/pb` directory
- Better for high-throughput scenarios

**Faber gap:**
- REST only

---

### 6. Extended Resource Limits (Medium Priority)

**go-judge additional limits:**
```typescript
interface Cmd {
  cpuRateLimit?: number;      // CPU throttling (1000 = 1 core)
  cpuSetLimit?: string;       // CPU affinity
  stackLimit?: number;        // Stack size limit
  dataSegmentLimit?: boolean; // rlimit_data
  addressSpaceLimit?: boolean; // rlimit_address_space
}
```

**Faber gap:**
- No CPU rate limiting (only quota)
- No CPU affinity
- No stack limit
- No rlimit support

---

### 7. Result Status Codes (Low Priority)

**go-judge status enum:**
```typescript
enum Status {
  Accepted = 'Accepted',
  MemoryLimitExceeded = 'Memory Limit Exceeded',
  TimeLimitExceeded = 'Time Limit Exceeded',
  OutputLimitExceeded = 'Output Limit Exceeded',
  FileError = 'File Error',
  NonzeroExitStatus = 'Nonzero Exit Status',
  Signalled = 'Signalled',
  InternalError = 'Internal Error',
}
```

**Faber gap:**
- Only returns exit_code, no semantic status
- No distinction between TLE, MLE, etc.

---

### 8. Prometheus Metrics (Medium Priority)

**go-judge metrics:**
- Worker goroutine count
- Queue length
- Container count
- CPU/Memory usage per execution
- Cache filesystem usage

**Faber gap:**
- No metrics endpoint
- No observability

---

### 9. Mount Configuration (Low Priority)

**go-judge implementation:**
- `mount.yaml` for custom mount points
- Fine-grained control over bind mounts
- Per-mount options (ro, rw, size)

**Faber gap:**
- Hardcoded mount points
- No runtime configuration

---

### 10. Shell Tool (Low Priority)

**go-judge feature:**
- `go-judge-shell` binary for debugging
- Opens interactive shell inside container
- Useful for development/testing

**Faber gap:**
- No debugging tool

---

## Implementation Roadmap

### Phase 1: File Management (High Priority)
1. **FileStore service**
   - In-memory file storage with TTL
   - Thread-safe HashMap with cleanup worker

2. **File API endpoints**
   - `POST /api/v1/file` - Upload file
   - `GET /api/v1/file` - List files
   - `GET /api/v1/file/:id` - Download file
   - `DELETE /api/v1/file/:id` - Delete file

3. **Task copyOut support**
   - Add `copy_out: Vec<String>` to Task
   - Add `copy_out_cached: Vec<String>` to Task
   - Return files in response

4. **PreparedFile reference**
   - Add `file_id` variant to files field
   - Resolve fileId to content at execution time

### Phase 2: Enhanced Execution (Medium Priority)
1. **Pipe mapping**
   - Inter-process pipes
   - Content proxy/capture

2. **Extended limits**
   - CPU rate limiting via cgroup
   - Output size limits
   - Stack limits via rlimit

3. **Better status reporting**
   - Semantic status enum
   - Distinguish TLE/MLE/OLE

### Phase 3: Transport & Monitoring (Medium Priority)
1. **WebSocket support**
   - Async execution
   - Real-time streaming

2. **Prometheus metrics**
   - Execution stats
   - Resource usage

3. **gRPC API**
   - Parallel to REST
   - Protobuf definitions

### Phase 4: Advanced Features (Low Priority)
1. **Seccomp filtering**
2. **User namespace**
3. **Mount configuration**
4. **Debug shell tool**

---

## Architecture Differences

### go-judge Architecture
```
Transport (HTTP/WS/gRPC/FFI)
         ↓
Sandbox Worker (Pool)
         ↓
EnvExec (Execution Engine)
         ↓
Platform (Linux/Windows/macOS)
```

- **Environment Pool**: Pre-created container environments
- **Worker Pool**: Concurrent request handling
- **File Store**: Shared memory file cache

### Faber Architecture
```
HTTP (Axum)
    ↓
Execute Handler
    ↓
Runtime (Container + Cgroup)
    ↓
Linux Namespaces
```

- **Simpler design**: No pooling, fresh container per request
- **No file store**: Stateless execution
- **Single transport**: REST only

---

## Code Organization Comparison

### go-judge
```
cmd/           - Entry points (go-judge, go-judge-shell, go-judge-ffi)
env/           - Environment/container management
envexec/       - Execution engine
filestore/     - File caching system
worker/        - Request handling pool
pb/            - Protobuf definitions
```

### Faber
```
src/           - Main binary
crates/
  faber-api/   - HTTP API
  faber-runtime/ - Execution engine
    container/ - Namespace isolation
    cgroup/    - Resource limits
    runtime/   - Task execution
```

---

## Recommended Priority Implementation

1. **copyOut + File Store** - Essential for compile-and-run workflows
2. **Status enum** - Better error reporting
3. **Output limits** - Prevent runaway output
4. **Metrics** - Production observability
5. **WebSocket** - Real-time streaming
6. **Pipe mapping** - Interactive problems
