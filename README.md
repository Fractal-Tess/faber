# Faber

<img src="faber.png" alt="Faber Logo" width="200">

[![Build and Push Docker Image](https://github.com/Fractal-Tess/faber/actions/workflows/docker-build-push.yml/badge.svg?branch=main)](https://github.com/Fractal-Tess/faber/actions/workflows/docker-build-push.yml)

A secure, sandboxed task task execution runtime that runs commands in isolated containers with resource limits and monitoring.

## Progress

- [x] **Runtime**:

  - [x] Cgroups
  - [x] Namespaces
  - [x] Timeouts (partial)
  - [x] Resource usage reporting (partial - kind of buggy at the moment)
  - [x] Parallel execution
  - [x] Sequential execution
  - [x] Dropped capabilities
  - [x] Unprivileged user
  - [ ] Syscall filtering
  - [ ] Step caching
  - [ ] Not require root privileges (currently needed to create dev devices)

- [x] **API**:

  - [x] API request hash caching (reqest fingerprint)

- [ ] **Docs**:

  - [ ] API docs
  - [ ] Runtime docs

- [ ] **SDKs**:

  - [ ] JS/TS
  - [ ] PHP
  - [ ] Python
  - [ ] Go
  - [ ] Rust

## API Documentation

### Authentication

**Current Status:** The Faber API does not require authentication. Both endpoints are publicly accessible.

> **Note:** This is suitable for development and trusted networks. For production deployments, consider adding authentication middleware or placing the API behind a reverse proxy with authentication.

### Endpoints

#### Health Check

Check if the Faber API is running.

```bash
GET /api/v1/health
```

**Response (200 OK):**
```json
{
  "status": "healthy"
}
```

#### Execute Tasks

Execute a sequence or parallel group of tasks in an isolated container environment.

```bash
POST /api/v1/execute
Content-Type: application/json
```

**Request Body:**

The request body is an array of execution steps. Each step can be either:
- **Single Task**: An object representing a single task to execute
- **Parallel Tasks**: An array of tasks to execute in parallel

**Task Object Structure:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cmd` | string | Yes | Command to execute (e.g., `/usr/bin/gcc`, `./hello`) |
| `args` | string[] | No | Command arguments |
| `env` | object | No | Environment variables as key-value pairs |
| `stdin` | string | No | Standard input content to provide to the command |
| `files` | object | No | Files to create before execution (path → content) |
| `working_dir` | string | No | Working directory for the command |

**Request Example:**

```json
[
  {
    "cmd": "/usr/bin/gcc",
    "args": ["hello.c", "-o", "hello"],
    "files": {
      "hello.c": "#include <stdio.h>\n\nint main() {\n    printf(\"Hello, World!\\n\");\n    return 0;\n}\n"
    }
  },
  {
    "cmd": "./hello"
  },
  [
    {
      "cmd": "echo",
      "args": ["task1"]
    },
    {
      "cmd": "echo",
      "args": ["task2"]
    }
  ]
]
```

In this example:
- Step 1: Compiles `hello.c` (single task)
- Step 2: Runs the compiled binary (single task)
- Step 3: Runs two echo commands in parallel (parallel tasks)

**Response (200 OK):**

The response is an array of execution step results, mirroring the request structure.

**TaskResult Object Structure (Completed):**

| Field | Type | Description |
|-------|------|-------------|
| `stdout` | string | Standard output from the command |
| `stderr` | string | Standard error from the command |
| `exit_code` | number | Exit code (0 = success) |
| `stats` | object | Resource usage statistics |

**TaskResult Object Structure (Failed):**

| Field | Type | Description |
|-------|------|-------------|
| `error` | string | Error message describing what went wrong |
| `stats` | object | Resource usage statistics |

**TaskStats Object Structure:**

| Field | Type | Description |
|-------|------|-------------|
| `memory_peak_bytes` | number | Peak memory usage in bytes |
| `cpu_usage_percent` | number | CPU usage percentage |
| `pids_peak` | number | Peak number of processes |
| `execution_time_ms` | number | Execution time in milliseconds |

**Response Example:**

```json
[
  {
    "stdout": "",
    "stderr": "",
    "exit_code": 0,
    "stats": {
      "memory_peak_bytes": 5767168,
      "cpu_usage_percent": 16722,
      "pids_peak": 3,
      "execution_time_ms": 118
    }
  },
  {
    "stdout": "Hello, World!\n",
    "stderr": "",
    "exit_code": 0,
    "stats": {
      "memory_peak_bytes": 0,
      "cpu_usage_percent": 0,
      "pids_peak": 0,
      "execution_time_ms": 17
    }
  },
  [
    {
      "stdout": "task1\n",
      "stderr": "",
      "exit_code": 0,
      "stats": {
        "memory_peak_bytes": 0,
        "cpu_usage_percent": 0,
        "pids_peak": 0,
        "execution_time_ms": 5
      }
    },
    {
      "stdout": "task2\n",
      "stderr": "",
      "exit_code": 0,
      "stats": {
        "memory_peak_bytes": 0,
        "cpu_usage_percent": 0,
        "pids_peak": 0,
        "execution_time_ms": 5
      }
    }
  ]
]
```

**Error Responses:**

- **400 Bad Request**: Empty task group
- **500 Internal Server Error**: Container setup or execution failure

## Data Structures

This section documents the core data structures used in the Faber runtime. These types are defined in the `faber-runtime` crate and are used for both API requests and responses.

### Task

Represents a single command to execute in an isolated container.

```rust
pub struct Task {
    pub cmd: String,                          // Required: Command to execute
    pub args: Option<Vec<String>>,            // Optional: Command arguments
    pub env: Option<HashMap<String, String>>, // Optional: Environment variables
    pub stdin: Option<String>,                // Optional: Standard input content
    pub files: Option<HashMap<String, String>>, // Optional: Files to create (path → content)
    pub working_dir: Option<String>,          // Optional: Working directory
}
```

**Fields:**
- `cmd`: The command to execute (e.g., `/usr/bin/gcc`, `./hello`, `python`)
- `args`: List of command-line arguments to pass to the command
- `env`: Key-value pairs of environment variables to set for the command
- `stdin`: String content to provide as standard input to the command
- `files`: Files to create in the container before execution (mapping file paths to their contents)
- `working_dir`: The working directory in which to execute the command

### ExecutionStep

Represents either a single task or multiple tasks to execute in parallel.

```rust
pub enum ExecutionStep {
    Single(Task),
    Parallel(Vec<Task>),
}
```

**Variants:**
- `Single(Task)`: Executes a single task
- `Parallel(Vec<Task>)`: Executes multiple tasks concurrently

**Serialization:** When serializing to JSON:
- `Single` variants are serialized as the task object itself
- `Parallel` variants are serialized as an array of task objects

This allows for concise and intuitive API requests where an object represents a single task and an array represents parallel tasks.

### TaskResult

Represents the result of executing a task, either successful completion or failure.

```rust
pub enum TaskResult {
    Completed {
        stdout: String,
        stderr: String,
        exit_code: i32,
        stats: TaskResultStats,
    },
    Failed {
        error: String,
        stats: TaskResultStats,
    },
}
```

**Variants:**

**Completed:**
- `stdout`: Standard output captured from the command
- `stderr`: Standard error captured from the command
- `exit_code`: The process exit code (0 typically indicates success)
- `stats`: Resource usage statistics during execution

**Failed:**
- `error`: A message describing what went wrong
- `stats`: Resource usage statistics collected before failure

### TaskResultStats

Contains resource usage statistics collected during task execution.

```rust
pub struct TaskResultStats {
    pub memory_peak_bytes: u64,     // Peak memory usage in bytes
    pub cpu_usage_percent: u64,     // CPU usage percentage (scaled by 100)
    pub pids_peak: u64,             // Peak number of processes/threads
    pub execution_time_ms: u64,     // Execution time in milliseconds
}
```

**Fields:**
- `memory_peak_bytes`: Maximum memory used during execution (in bytes)
- `cpu_usage_percent`: CPU utilization as a percentage (scaled by 100, so 10000 = 100%)
- `pids_peak`: Maximum number of processes/threads used during execution
- `execution_time_ms`: Total execution time in milliseconds

### Type Aliases

```rust
pub type TaskGroup = Vec<ExecutionStep>;
pub type TaskGroupResult = Vec<ExecutionStepResult>;
```

- `TaskGroup`: A sequence of execution steps (runs sequentially)
- `TaskGroupResult`: Results corresponding to each execution step

### Example: Complete Data Flow

```rust
// Request: A TaskGroup with 3 steps
// Step 1: Single task (compile)
// Step 2: Single task (run)
// Step 3: Parallel tasks (multiple checks)

let request: TaskGroup = vec![
    ExecutionStep::Single(Task { /* compile */ }),
    ExecutionStep::Single(Task { /* run */ }),
    ExecutionStep::Parallel(vec![
        Task { /* check 1 */ },
        Task { /* check 2 */ },
    ]),
];

// Response: TaskGroupResult with corresponding results
let response: TaskGroupResult = vec![
    ExecutionStepResult::Single(TaskResult::Completed { /* ... */ }),
    ExecutionStepResult::Single(TaskResult::Completed { /* ... */ }),
    ExecutionStepResult::Parallel(vec![
        TaskResult::Completed { /* ... */ },
        TaskResult::Completed { /* ... */ },
    ]),
];
```

## Quick Start

### Using Docker (Recommended)

Create your own dokcer image that uses the base faber image as a base and add needed compilers or interprters. Here is a `C` lang example:

```docker
FROM vgfractal/faber AS faber
FROM debian:latest

# install compilers
RUN apt update && apt install -y \
    gcc \
    make \
    libc-dev

WORKDIR /opt
COPY --from=faber /opt/faber /opt

EXPOSE 3000/tcp
ENTRYPOINT ["./faber"]
```

```bash
sudo docker build -t custom-faber .
sudo docker run --privileged --cgroupns=host -p 3000:3000 custom-faber
```

### Example: Running a C Program

Once your Faber container is running, you can execute tasks. Here's an example of compiling and running a simple C program:

```c
// hello.c
#include <stdio.h>

int main() {
    printf("Hello, World!\n");
    return 0;
}
```

Send a POST request to execute this C program:

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -d '[
    {
      "cmd": "/usr/bin/gcc",
      "args": [
        "hello.c",
        "-o",
        "hello"
      ],
      "files": {
        "hello.c": "#include <stdio.h>\n\nint main() {\n    printf(\"Hello, World!\\n\");\n    return 0;\n}\n"
      }
    },
    {
      "cmd": "./hello"
    }
  ]'
```

**Expected Output:**

```json
[
  {
    "stdout": "",
    "stderr": "",
    "exit_code": 0,
    "stats": {
      "memory_peak_bytes": 5767168,
      "cpu_usage_percent": 16722,
      "pids_peak": 3,
      "execution_time_ms": 118
    }
  },
  {
    "stdout": "Hello, World!\n",
    "stderr": "",
    "exit_code": 0,
    "stats": {
      "memory_peak_bytes": 0,
      "cpu_usage_percent": 0,
      "pids_peak": 0,
      "execution_time_ms": 17
    }
  }
]
```
