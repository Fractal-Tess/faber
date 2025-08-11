#!/usr/bin/env bash
set -Eeuo pipefail

# Simple cgroup diagnostic script for containers (L2) and short-lived workloads (L3)
# Logs to /var/log/faber by default

umask 022

LOG_DIR=${FABER_LOG_DIR:-/var/log/faber}
TS=$(date +%Y%m%d-%H%M%S)
LOG_FILE="${LOG_DIR}/cgroup-debug-${TS}.log"

mkdir -p "${LOG_DIR}"

log() {
  local level="$1"; shift
  printf '[%s] [%s] %s\n' "$(date -Iseconds)" "${level}" "$*" | tee -a "${LOG_FILE}"
}

section() {
  printf '\n==== %s ====%s\n' "$1" "" | tee -a "${LOG_FILE}"
}

ok()   { log OK   "$*"; }
warn() { log WARN "$*"; }
err()  { log ERR  "$*"; }

on_err() {
  err "Command failed at line ${BASH_LINENO[0]}: ${BASH_COMMAND}"
}
trap on_err ERR

is_writable_dir() {
  local d="$1"
  [[ -d "$d" ]] || return 1
  test_file="${d}/.cgtest.$$"
  if ( : >"${test_file}" ) 2>/dev/null; then
    rm -f "${test_file}" || true
    return 0
  fi
  return 1
}

in_list() {
  # in_list "needle" "space separated list"
  local needle="$1"; shift
  for item in $*; do
    [[ "$item" == "$needle" ]] && return 0
  done
  return 1
}

get_cg2_mount() {
  awk '$0 ~ / - cgroup2 / {print $5; exit}' /proc/self/mountinfo
}

get_cg1_mount_for() {
  local controller="$1"
  awk -v c="${controller}" '$0 ~ / - cgroup / { if ($0 ~ (" " c "(,| |$)")) print $5 }' /proc/self/mountinfo | head -n1
}

show_basic_env() {
  section "Environment"
  log INFO "Hostname: $(hostname)"
  log INFO "Kernel: $(uname -a)"
  if [[ -f /etc/os-release ]]; then
    log INFO "OS: $(. /etc/os-release; echo "${PRETTY_NAME:-unknown}")"
  fi
  log INFO "User: $(id)"
  log INFO "CWD: $(pwd)"
  log INFO "Inside container (heuristic): $(grep -qi docker /proc/1/cgroup && echo yes || echo unknown)"
  log INFO "cgroup ns: self=$(readlink /proc/self/ns/cgroup) init=$(readlink /proc/1/ns/cgroup)"
}

show_mounts() {
  section "Mounts"
  log INFO "cgroup entries in /proc/filesystems:"
  grep -E '^nodev\s+cgroup2?$' /proc/filesystems | tee -a "${LOG_FILE}" || true
  log INFO "cgroup mounts from /proc/self/mountinfo:"
  awk '$0 ~ / - cgroup2 | - cgroup / {print}' /proc/self/mountinfo | tee -a "${LOG_FILE}" || true
}

cg_version=0

detect_version() {
  section "Detect cgroup version"
  if awk '$0 ~ /nodev\s+cgroup2$/' /proc/filesystems >/dev/null 2>&1; then
    cg_version=2
    ok "Detected cgroup v2 support in kernel"
  else
    cg_version=1
    warn "cgroup v2 not listed; assuming cgroup v1"
  fi

  # If a cgroup2 mount exists, prefer that
  if grep -q ' - cgroup2 ' /proc/self/mountinfo; then
    cg_version=2
    ok "Found mounted cgroup v2 hierarchy"
  else
    warn "No cgroup v2 mount detected; will probe cgroup v1 controllers"
  fi
}

probe_v2() {
  section "cgroup v2 probe"
  local mnt
  mnt=$(get_cg2_mount || true)
  if [[ -z "${mnt}" ]]; then
    err "No cgroup v2 mount found"
    return 1
  fi
  log INFO "cgroup v2 mount: ${mnt}"
  if is_writable_dir "${mnt}"; then ok "${mnt} is writable"; else warn "${mnt} not writable"; fi

  local controllers subtree
  controllers=$(cat "${mnt}/cgroup.controllers" 2>/dev/null || true)
  subtree=$(cat "${mnt}/cgroup.subtree_control" 2>/dev/null || true)
  log INFO "controllers: ${controllers:-<empty>}"
  log INFO "subtree_control: ${subtree:-<empty>}"

  for need in memory cpu pids; do
    if in_list "${need}" ${controllers}; then
      ok "controller available: ${need}"
    else
      warn "controller NOT listed in cgroup.controllers: ${need}"
    fi
  done

  # Note: enabling controllers requires parent delegation; log state
  for need in memory cpu pids; do
    if in_list "+${need}" ${subtree}; then
      ok "controller enabled for children: ${need}"
    else
      warn "controller NOT enabled in cgroup.subtree_control: ${need}"
    fi
  done

  # Optionally try to delegate and enable controllers
  if [[ ${DELEGATE:-0} -eq 1 ]]; then
    section "cgroup v2 delegation attempt"
    if [[ -w "${mnt}/cgroup.subtree_control" ]]; then
      local leaf="${mnt}/faber-container"
      mkdir -p "${leaf}" 2>/dev/null || true
      # Move all tasks from root to the leaf until root is empty or max passes reached
      local pass=0 moved any=0
      while [[ -s "${mnt}/cgroup.procs" && ${pass} -lt 5 ]]; do
        any=0
        while read -r pid; do
          [[ -z "${pid}" ]] && continue
          if echo "${pid}" > "${leaf}/cgroup.procs" 2>/dev/null; then any=1; fi
        done < "${mnt}/cgroup.procs"
        pass=$((pass+1))
        log INFO "delegation pass ${pass}: root has $(wc -l < "${mnt}/cgroup.procs") procs remaining"
        [[ ${any} -eq 0 ]] && break
      done
      if [[ ! -s "${mnt}/cgroup.procs" ]]; then
        ok "root cgroup has no processes; attempting to enable controllers"
      else
        warn "root cgroup still has processes; enabling controllers may fail"
      fi
      for need in memory cpu pids; do
        if echo "+${need}" > "${mnt}/cgroup.subtree_control" 2>/dev/null; then
          ok "enabled ${need} in subtree_control"
        else
          warn "failed to enable ${need} in subtree_control"
        fi
      done
      subtree=$(cat "${mnt}/cgroup.subtree_control" 2>/dev/null || true)
      log INFO "subtree_control (after): ${subtree:-<empty>}"
    else
      warn "cgroup.subtree_control not writable; cannot delegate"
    fi
  fi

  # Try creating a test cgroup and writing some limits
  local testcg="${mnt}/faber-debug-$$"
  section "cgroup v2 create-test: ${testcg}"
  if mkdir -p "${testcg}" 2>/dev/null; then
    ok "created ${testcg}"
  else
    err "failed to create ${testcg} (check write perms and delegation)"
    return 0
  fi

  # Write light-touch limits where possible; restore to defaults afterwards
  if [[ -w "${testcg}/memory.max" ]]; then
    local prev_mem
    prev_mem=$(cat "${testcg}/memory.max" 2>/dev/null || echo "max")
    if echo 268435456 > "${testcg}/memory.max" 2>/dev/null; then
      ok "memory.max set to 256M"
      echo "${prev_mem}" > "${testcg}/memory.max" 2>/dev/null || true
    else
      warn "unable to write memory.max"
    fi
  else
    warn "memory.max not writable"
  fi

  if [[ -w "${testcg}/pids.max" ]]; then
    local prev_pids
    prev_pids=$(cat "${testcg}/pids.max" 2>/dev/null || echo "max")
    if echo 512 > "${testcg}/pids.max" 2>/dev/null; then
      ok "pids.max set to 512"
      echo "${prev_pids}" > "${testcg}/pids.max" 2>/dev/null || true
    else
      warn "unable to write pids.max"
    fi
  else
    warn "pids.max not writable"
  fi

  if [[ -w "${testcg}/cpu.max" ]]; then
    local prev_cpu
    prev_cpu=$(cat "${testcg}/cpu.max" 2>/dev/null || echo "max 100000")
    if echo "20000 100000" > "${testcg}/cpu.max" 2>/dev/null; then
      ok "cpu.max set to 20% (20000/100000)"
      echo "${prev_cpu}" > "${testcg}/cpu.max" 2>/dev/null || true
    else
      warn "unable to write cpu.max"
    fi
  else
    warn "cpu.max not writable"
  fi

  # Spawn a trivial child and try to move it into the test cgroup
  local child_pid
  ( sleep 0.5 ) &
  child_pid=$!
  if [[ -w "${testcg}/cgroup.procs" ]]; then
    if echo "${child_pid}" > "${testcg}/cgroup.procs" 2>/dev/null; then
      ok "moved pid ${child_pid} to ${testcg}"
    else
      warn "failed to move pid ${child_pid} to ${testcg}"
    fi
  else
    warn "${testcg}/cgroup.procs not writable"
  fi
  wait "${child_pid}" 2>/dev/null || true

  # Show some stats
  for f in cgroup.events cgroup.stat cpu.stat memory.events memory.current memory.high memory.max pids.current pids.max; do
    if [[ -f "${testcg}/${f}" ]]; then
      printf '%s: %s\n' "${f}" "$(tr '\n' ';' < "${testcg}/${f}")" | tee -a "${LOG_FILE}" || true
    fi
  done

  # Cleanup
  if rmdir "${testcg}" 2>/dev/null; then
    ok "removed ${testcg}"
  else
    warn "could not remove ${testcg} (might not be empty or busy)"
  fi
}

probe_v1() {
  section "cgroup v1 probe"
  local ctrs="cpuset cpu cpuacct memory pids"
  for ctr in ${ctrs}; do
    local mnt
    mnt=$(get_cg1_mount_for "${ctr}" || true)
    if [[ -z "${mnt}" ]]; then
      warn "controller ${ctr}: mount not found"
      continue
    fi
    log INFO "${ctr} mount: ${mnt}"
    if is_writable_dir "${mnt}"; then ok "${mnt} writable"; else warn "${mnt} not writable"; fi

    local testcg="${mnt}/faber-debug-$$"
    if mkdir -p "${testcg}" 2>/dev/null; then
      ok "${ctr}: created ${testcg}"
    else
      warn "${ctr}: failed to create ${testcg}"
      continue
    fi

    case "${ctr}" in
      memory)
        if [[ -w "${testcg}/memory.limit_in_bytes" ]]; then
          local prev
          prev=$(cat "${testcg}/memory.limit_in_bytes" 2>/dev/null || echo 9223372036854771712)
          echo 268435456 > "${testcg}/memory.limit_in_bytes" 2>/dev/null && ok "memory.limit_in_bytes set to 256M" || warn "cannot write memory.limit_in_bytes"
          echo "${prev}" > "${testcg}/memory.limit_in_bytes" 2>/dev/null || true
        else
          warn "memory.limit_in_bytes not writable"
        fi
        ;;
      cpu)
        if [[ -w "${testcg}/cpu.cfs_quota_us" && -w "${testcg}/cpu.cfs_period_us" ]]; then
          local prev_q prev_p
          prev_q=$(cat "${testcg}/cpu.cfs_quota_us" 2>/dev/null || echo -1)
          prev_p=$(cat "${testcg}/cpu.cfs_period_us" 2>/dev/null || echo 100000)
          echo 20000 > "${testcg}/cpu.cfs_quota_us" 2>/dev/null && echo 100000 > "${testcg}/cpu.cfs_period_us" 2>/dev/null && ok "cpu quota set to 20%" || warn "cannot write cpu quota"
          echo "${prev_q}" > "${testcg}/cpu.cfs_quota_us" 2>/dev/null || true
          echo "${prev_p}" > "${testcg}/cpu.cfs_period_us" 2>/dev/null || true
        else
          warn "cpu quota files not writable"
        fi
        ;;
      pids)
        if [[ -w "${testcg}/pids.max" ]]; then
          local prev
          prev=$(cat "${testcg}/pids.max" 2>/dev/null || echo max)
          echo 512 > "${testcg}/pids.max" 2>/dev/null && ok "pids.max set to 512" || warn "cannot write pids.max"
          echo "${prev}" > "${testcg}/pids.max" 2>/dev/null || true
        else
          warn "pids.max not writable"
        fi
        ;;
    esac

    # Move a trivial child into v1 cgroup if possible
    ( sleep 0.5 ) &
    local child_pid=$!
    if [[ -w "${testcg}/tasks" ]]; then
      echo "${child_pid}" > "${testcg}/tasks" 2>/dev/null && ok "moved pid ${child_pid} to ${testcg}" || warn "failed to move pid ${child_pid}"
    fi
    wait "${child_pid}" 2>/dev/null || true

    rmdir "${testcg}" 2>/dev/null || warn "could not remove ${testcg}"
  done
}

maybe_run_tests() {
  if [[ ${RUN_TESTS:-0} -eq 0 ]]; then
    section "Light tests skipped"
    log INFO "Pass --run-tests to attempt short stress tests"
    return 0
  fi

  section "Light functional tests"
  # CPU test: constrain to ~20% for a brief busy loop
  if [[ ${cg_version} -eq 2 ]]; then
    local mnt testcg
    mnt=$(get_cg2_mount || true)
    if [[ -n "${mnt}" ]]; then
      testcg="${mnt}/faber-test-$$"
      mkdir -p "${testcg}" || true
      echo "20000 100000" > "${testcg}/cpu.max" 2>/dev/null || true
      ( bash -c 't0=$(date +%s); while [[ $(($(date +%s)-t0)) -lt 2 ]]; do :; done' ) &
      local pid=$!
      echo "${pid}" > "${testcg}/cgroup.procs" 2>/dev/null || true
      wait "${pid}" 2>/dev/null || true
      rmdir "${testcg}" 2>/dev/null || true
      ok "CPU quick test completed"
    fi
  fi
}

usage() {
  echo "Usage: $0 [--run-tests] [--delegate]"
}

RUN_TESTS=0
DELEGATE=0
if [[ $# -gt 0 ]]; then
  while [[ $# -gt 0 ]]; do
    case "${1:-}" in
      --run-tests) RUN_TESTS=1 ;;
      --delegate) DELEGATE=1 ;;
      -h|--help) usage; exit 0 ;;
      *) usage; exit 1 ;;
    esac
    shift
  done
fi

section "Start"
log INFO "Log file: ${LOG_FILE}"

show_basic_env
show_mounts

if [[ $(id -u) -ne 0 ]]; then
  warn "Not running as root; some operations may fail"
fi

if [[ ! -w "${LOG_DIR}" ]]; then
  warn "Log dir ${LOG_DIR} not writable; output may be partial"
fi

if [[ ! -e /proc/self/cgroup ]]; then
  err "/proc/self/cgroup missing"
fi

log INFO "/proc/self/cgroup:\n$(cat /proc/self/cgroup)"

if [[ -f /proc/cgroups ]]; then
  log INFO "/proc/cgroups:\n$(cat /proc/cgroups)"
fi

detect_version

if [[ ${cg_version} -eq 2 ]]; then
  probe_v2 || true
else
  probe_v1 || true
fi

maybe_run_tests || true

section "Done"
ok "Diagnostics complete. Attach log: ${LOG_FILE}" 