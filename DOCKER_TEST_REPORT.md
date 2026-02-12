# Faber Docker Testing Report

**Date:** 2026-02-12
**Tester:** Hephaestus
**Environment:** Linux with rootless Docker
**Faber Version:** Latest from main branch

## Executive Summary

The Faber application was extensively tested in Docker. The core functionality works well for basic command execution, file operations, and API authentication. However, several issues were discovered that affect reliability and usability.

## Test Environment Setup

### Docker Image Build
- **Status:** ✓ SUCCESS
- The production Dockerfile builds successfully using multi-stage build
- Final image size is minimal (debian:bookworm-slim base)

### Container Runtime Requirements
- **Privileged mode:** REQUIRED - for namespace and cgroup operations
- **Cgroup namespace:** MUST use `host` mode (not private)
- **Cgroup volume:** MUST mount `/sys/fs/cgroup` from host with read-write access
- **Docker execution:** MUST use `sudo docker` (rootless Docker has permission issues with cgroups)

### Required Setup Commands
```bash
# 1. Create and configure faber cgroup directory
sudo mkdir -p /sys/fs/cgroup/faber
sudo chmod 777 /sys/fs/cgroup/faber
echo "+cpu +memory +pids" | sudo tee /sys/fs/cgroup/faber/cgroup.subtree_control

# 2. Run container with proper permissions
sudo docker run -d \
  --privileged \
  --cgroupns=host \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  -e API_KEY=test-key-123 \
  -p 3333:3000 \
  faber-test:latest
```

## Test Results

### ✓ PASSING TESTS

#### 1. Health Check Endpoint
- **Endpoint:** `GET /api/v1/health`
- **Result:** Returns `{"status":"ok"}`
- **Status:** ✓ PASS

#### 2. Basic Command Execution
- **Command:** `/usr/bin/echo "Hello"`
- **Result:** Correct stdout, exit code 0
- **Resource Stats:** Memory, CPU, PIDs tracked correctly
- **Status:** ✓ PASS

#### 3. File Operations
- **Test:** Create file via `files` field, read with `cat`
- **Result:** File content correctly passed and read
- **Status:** ✓ PASS

#### 4. Multiple Files
- **Test:** Create multiple files in single request
- **Result:** All files created and accessible
- **Status:** ✓ PASS

#### 5. Exit Code Verification
- **Test:** `/usr/bin/true` (exit 0) and `/usr/bin/false` (exit 1)
- **Result:** Correct exit codes returned
- **Status:** ✓ PASS

#### 6. Stderr Capture
- **Test:** Commands that write to stderr
- **Result:** Stderr captured in response
- **Status:** ✓ PASS

#### 7. Command Not Found
- **Test:** Non-existent command
- **Result:** Exit code 127 returned
- **Status:** ✓ PASS

#### 8. Resource Stats
- **Metrics Tracked:**
  - `memory_peak_bytes` - Peak memory usage
  - `cpu_usage_usec` - CPU time in microseconds
  - `pids_peak` - Peak process count
  - `execution_time_ms` - Wall clock execution time
- **Status:** ✓ PASS

#### 9. API Key Authentication - Missing Key
- **Test:** Request without Authorization header
- **Result:** HTTP 401 Unauthorized
- **Status:** ✓ PASS

#### 10. API Key Authentication - Wrong Key
- **Test:** Request with incorrect API key
- **Result:** HTTP 401 Unauthorized
- **Status:** ✓ PASS

### ✗ FAILING/PROBLEMATIC TESTS

#### 1. Sequential Execution
- **Issue:** Second and subsequent commands fail with "ENOMEM: Out of memory"
- **Root Cause:** Task cgroup directories are not being properly cleaned up after first command
- **Evidence:** Leftover directories in `/sys/fs/cgroup/faber/task-*`
- **Impact:** HIGH - breaks multi-step workflows

#### 2. Parallel Execution
- **Issue:** Server returns HTTP 500 Internal Server Error
- **Root Cause:** Likely related to cgroup management issues
- **Impact:** HIGH - parallel execution is a core feature

#### 3. C Program Compilation (from README example)
- **Issue:** gcc not available in production image
- **Exit Code:** 127 (command not found)
- **Note:** The README example assumes a custom image with gcc installed
- **Impact:** MEDIUM - documentation/example mismatch

#### 4. Container Stability
- **Issue:** Container crashes/exits after certain operations
- **Symptoms:** 
  - Parallel execution causes immediate crash
  - Multiple sequential commands cause eventual crash
- **Impact:** HIGH - affects reliability

#### 5. Cgroup Cleanup
- **Issue:** Task cgroup directories not removed after execution
- **Location:** `/sys/fs/cgroup/faber/task-*`
- **Impact:** MEDIUM - resource leak, requires manual cleanup

## Issues Discovered

### Critical Issues

1. **Cgroup Permission Issues with Rootless Docker**
   - Rootless Docker maps container root to host user UID
   - Cgroup operations require true root privileges
   - **Workaround:** Use `sudo docker` instead of rootless Docker

2. **Task Cgroup Cleanup Failure**
   - Task directories persist after execution
   - Subsequent executions fail with ENOMEM
   - **Workaround:** Manual cleanup: `sudo rmdir /sys/fs/cgroup/faber/task-*`

3. **Parallel Execution Crashes Server**
   - HTTP 500 error on parallel task requests
   - Container exits/crashes
   - **No workaround available**

### Medium Issues

4. **Documentation/Example Mismatch**
   - README C compilation example requires gcc
   - Production image doesn't include gcc
   - **Suggestion:** Update README to clarify custom image requirements

5. **Missing API Documentation**
   - No OpenAPI/Swagger docs
   - Authentication header format not documented
   - **Note:** Found `Authorization: Bearer <key>` format by reading source code

### Minor Issues

6. **Test Script Issues**
   - Original `test-docker.sh` doesn't set API_KEY
   - Uses wrong Authorization header format
   - **Fixed:** Created `test-docker-modified.sh` with corrections

## Recommendations

### Immediate Actions

1. **Fix cgroup cleanup** - Ensure task directories are removed after execution
2. **Fix parallel execution** - Debug and fix the crash issue
3. **Fix sequential execution** - Ensure proper cleanup between steps

### Short-term Improvements

4. **Update documentation** - Clarify Docker runtime requirements
5. **Add health check to Dockerfile** - Use the existing `/api/v1/health` endpoint
6. **Create example custom images** - Provide Dockerfiles with common compilers (gcc, python, node)

### Long-term Improvements

7. **Add proper error handling** - Return meaningful error messages instead of empty responses
8. **Add request validation** - Validate command paths exist before execution
9. **Improve logging** - Add structured logging for debugging
10. **Add metrics endpoint** - Expose Prometheus metrics for monitoring

## Conclusion

Faber shows promise as a sandboxed execution runtime, but has critical stability issues that need to be addressed before production use. The cgroup management and cleanup issues are the most pressing concerns. With proper fixes, it could be a useful tool for isolated code execution.

## Test Artifacts

- **Modified test script:** `scripts/test-docker-modified.sh`
- **Comprehensive test suite:** `scripts/comprehensive-test.sh`
- **Test output logs:** Available in terminal history
