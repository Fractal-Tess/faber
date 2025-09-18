# Faber

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
curl -X POST http://localhost:3000/execute \
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
