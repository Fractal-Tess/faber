use faber_runtime::{
    CgroupConfigBuilder, ExecutionStep, ExecutionStepResult, RuntimeBuilder, RuntimeResult, Task,
    TaskResult,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

static SECURITY_TEST_LOCK: Mutex<()> = Mutex::new(());

const SECURITY_PROBE_SOURCE: &str = include_str!("fixtures/security_probe.c");

#[derive(Debug, Deserialize)]
struct RlimitState {
    soft: u64,
    hard: u64,
}

#[derive(Debug, Deserialize)]
struct SecurityState {
    pid: u32,
    ppid: u32,
    uid: u32,
    euid: u32,
    gid: u32,
    egid: u32,
    groups: Vec<u32>,
    namespaces: HashMap<String, u64>,
    uid_map: String,
    gid_map: String,
    status: String,
    cgroup: String,
    mountinfo: String,
    route4: String,
    route6: String,
    rlimits: HashMap<String, RlimitState>,
}

const PID_PROBE_SOURCE: &str = r#"
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    pid_t children[32];
    int started = 0;
    int fork_error = 0;

    while (started < 32) {
        pid_t child = fork();
        if (child == 0) {
            pause();
            _exit(0);
        }
        if (child < 0) {
            fork_error = errno;
            break;
        }
        children[started++] = child;
    }

    printf("%d %d\n", started, fork_error);
    fflush(stdout);

    for (int i = 0; i < started; i++) {
        kill(children[i], SIGKILL);
    }
    for (int i = 0; i < started; i++) {
        waitpid(children[i], NULL, 0);
    }

    return started < 32 && fork_error == EAGAIN ? 0 : 1;
}
"#;

const ORPHAN_PROBE_SOURCE: &str = r#"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(void) {
    pid_t child = fork();
    if (child < 0) {
        return 1;
    }
    if (child == 0) {
        int marker = open("orphan.pid", O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0644);
        if (marker < 0) {
            _exit(2);
        }
        dprintf(marker, "%d\n", getpid());
        close(marker);
        close(STDIN_FILENO);
        close(STDOUT_FILENO);
        close(STDERR_FILENO);
        pause();
        _exit(0);
    }

    for (int attempt = 0; attempt < 100; attempt++) {
        if (access("orphan.pid", F_OK) == 0) {
            return 0;
        }
        usleep(1000);
    }
    return 3;
}
"#;

const MEMORY_PROBE_SOURCE: &str = r#"
#include <stdlib.h>
#include <unistd.h>

int main(void) {
    const size_t allocation = 128UL * 1024UL * 1024UL;
    const long page_size = sysconf(_SC_PAGESIZE);
    volatile unsigned char *memory = malloc(allocation);
    if (memory == NULL) {
        return 2;
    }

    for (size_t offset = 0; offset < allocation; offset += (size_t)page_size) {
        memory[offset] = 1;
    }

    return 0;
}
"#;

fn lock_security_tests() -> MutexGuard<'static, ()> {
    SECURITY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn faber_cgroup_path() -> PathBuf {
    let membership = std::fs::read_to_string("/proc/self/cgroup")
        .expect("failed to read the test process cgroup membership");
    let relative_path = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .expect("test process is not in a cgroup v2 hierarchy");
    let own_path = PathBuf::from("/sys/fs/cgroup").join(relative_path.trim_start_matches('/'));

    own_path
        .ancestors()
        .map(|ancestor| ancestor.join("faber"))
        .find(|candidate| candidate.is_dir())
        .expect("failed to locate the Faber cgroup")
}

fn assert_no_task_cgroups() {
    let leaked: Vec<PathBuf> = std::fs::read_dir(faber_cgroup_path())
        .expect("failed to inspect the Faber cgroup")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("task-"))
        })
        .collect();

    assert!(
        leaked.is_empty(),
        "task cgroups leaked after execution: {leaked:?}"
    );
}

fn task(cmd: &str, args: &[&str]) -> Task {
    Task {
        cmd: cmd.to_string(),
        args: Some(args.iter().map(|arg| (*arg).to_string()).collect()),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
    }
}

fn task_with_file(cmd: &str, args: &[&str], path: &str, content: &str) -> Task {
    let mut files = HashMap::new();
    files.insert(path.to_string(), content.to_string());

    Task {
        files: Some(files),
        ..task(cmd, args)
    }
}

fn execute(tasks: Vec<Task>) -> Vec<ExecutionStepResult> {
    let task_group = tasks.into_iter().map(ExecutionStep::Single).collect();
    let result = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build()
        .execute()
        .expect("runtime execution failed");

    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();
    results
}

fn single_result(result: &ExecutionStepResult) -> &TaskResult {
    let ExecutionStepResult::Single(result) = result else {
        panic!("expected a single task result");
    };
    result
}

fn status_field<'a>(status: &'a str, name: &str) -> &'a str {
    status
        .lines()
        .find_map(|line| line.strip_prefix(name))
        .map(str::trim)
        .unwrap_or_else(|| panic!("missing {name} in /proc/self/status"))
}

fn mountinfo_line<'a>(mountinfo: &'a str, mountpoint: &str) -> &'a str {
    mountinfo
        .lines()
        .filter(|line| line.split_whitespace().nth(4) == Some(mountpoint))
        .last()
        .unwrap_or_else(|| panic!("missing {mountpoint} in mountinfo"))
}

fn namespace_inode(name: &str) -> u64 {
    std::fs::metadata(format!("/proc/self/ns/{name}"))
        .unwrap_or_else(|error| panic!("failed to inspect outer {name} namespace: {error}"))
        .ino()
}

#[test]
fn security_probe_records_identity_namespaces_mounts_and_limits() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task_with_file(
            "/usr/bin/gcc",
            &["security_probe.c", "-o", "security_probe"],
            "security_probe.c",
            SECURITY_PROBE_SOURCE,
        ),
        task("./security_probe", &[]),
    ]);

    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("security probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(
        *compile_exit, 0,
        "security probe did not compile: {compile_stderr}"
    );

    let TaskResult::Completed {
        stdout,
        stderr,
        exit_code,
        ..
    } = single_result(&results[1])
    else {
        panic!("security probe did not complete: {:?}", results[1]);
    };
    assert_eq!(*exit_code, 0, "security probe failed: {stderr}");

    let state: SecurityState =
        serde_json::from_str(stdout).expect("security probe emitted invalid JSON");
    assert!(
        state.pid > 1,
        "task replaced the namespace reaper: {state:?}"
    );
    assert!(state.ppid <= 1, "unexpected visible parent PID: {state:?}");
    assert_eq!((state.uid, state.euid), (65534, 65534));
    assert_eq!((state.gid, state.egid), (65534, 65534));

    for capability_set in ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"] {
        assert_eq!(
            status_field(&state.status, capability_set),
            "0000000000000000",
            "{capability_set} was not cleared"
        );
    }
    assert_eq!(status_field(&state.status, "NoNewPrivs:"), "1");
    status_field(&state.status, "Seccomp:")
        .parse::<u8>()
        .expect("Seccomp was not numeric");

    for namespace in ["mnt", "pid", "net", "uts", "ipc"] {
        let inner = state.namespaces[namespace];
        assert_ne!(
            inner,
            namespace_inode(namespace),
            "task did not enter a distinct {namespace} namespace"
        );
    }
    for namespace in ["user", "cgroup"] {
        assert!(
            state.namespaces[namespace] > 0,
            "missing {namespace} namespace evidence"
        );
    }

    assert!(!state.uid_map.trim().is_empty(), "missing UID map evidence");
    assert!(!state.gid_map.trim().is_empty(), "missing GID map evidence");
    assert!(
        state.groups.is_empty(),
        "supplementary groups were not cleared: {:?}",
        state.groups
    );
    assert!(
        state.cgroup.contains("task-"),
        "unexpected cgroup: {}",
        state.cgroup
    );
    assert!(!state.mountinfo.contains("/oldroot"));

    let root_mount = mountinfo_line(&state.mountinfo, "/");
    let root_optional_fields = root_mount
        .split(" - ")
        .next()
        .expect("root mountinfo lacked a separator");
    assert!(!root_optional_fields.contains("shared:"));
    assert!(!root_optional_fields.contains("master:"));

    let sys_mount = mountinfo_line(&state.mountinfo, "/sys");
    let sys_options = sys_mount
        .split_whitespace()
        .nth(5)
        .expect("sysfs mount options were missing");
    assert!(
        sys_options.split(',').any(|option| option == "ro"),
        "sysfs was not read-only: {sys_mount}"
    );
    assert!(
        state
            .mountinfo
            .lines()
            .all(|line| line.split_whitespace().nth(4) != Some("/sys/fs/cgroup")),
        "task unexpectedly retained a cgroup filesystem mount"
    );

    let _ = (&state.route4, &state.route6);

    for resource in ["cpu", "fsize", "nofile", "nproc", "stack", "core"] {
        let limit = state
            .rlimits
            .get(resource)
            .unwrap_or_else(|| panic!("missing {resource} rlimit evidence"));
        assert!(
            limit.soft <= limit.hard,
            "invalid {resource} rlimit: {limit:?}"
        );
    }
}

#[test]
fn filesystem_hides_outer_root_and_keeps_toolchains_read_only() {
    let _guard = lock_security_tests();
    let marker_path = format!("/root/faber-host-marker-{}", std::process::id());
    std::fs::write(&marker_path, "must remain outside the sandbox")
        .expect("failed to create outer-root marker");

    let script = format!(
        "test ! -e {marker_path} && test ! -e /oldroot && \
         ! touch /bin/faber-write-test 2>/dev/null && \
         ! touch /usr/bin/faber-write-test 2>/dev/null && \
         test ! -w /sys/fs/cgroup/cgroup.procs"
    );
    let results = execute(vec![task("/bin/sh", &["-c", &script])]);
    std::fs::remove_file(&marker_path).expect("failed to remove outer-root marker");

    let TaskResult::Completed {
        exit_code, stderr, ..
    } = single_result(&results[0])
    else {
        panic!("filesystem probe did not complete: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0, "filesystem boundary probe failed: {stderr}");
}

#[test]
fn submitted_files_reject_absolute_and_parent_paths() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task_with_file("/bin/true", &[], "/tmp/absolute-escape", "blocked"),
        task_with_file("/bin/true", &[], "../parent-escape", "blocked"),
    ]);

    for result in results {
        let TaskResult::Failed { error, .. } = single_result(&result) else {
            panic!("unsafe task file path was accepted: {result:?}");
        };
        assert!(
            error.contains("paths must be normalized and relative"),
            "unexpected path rejection: {error}"
        );
    }
}

#[test]
fn submitted_files_do_not_follow_workspace_symlinks() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task("/bin/ln", &["-s", "/tmp", "escape"]),
        task_with_file("/bin/true", &[], "escape/escaped.txt", "blocked"),
    ]);

    let TaskResult::Completed { exit_code, .. } = single_result(&results[0]) else {
        panic!("failed to create the adversarial symlink: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0);

    let TaskResult::Failed { error, .. } = single_result(&results[1]) else {
        panic!("workspace symlink was followed: {:?}", results[1]);
    };
    assert!(
        error.contains("without following links"),
        "unexpected symlink rejection: {error}"
    );
}

#[test]
fn output_streams_are_drained_bounded_and_report_truncation() {
    let _guard = lock_security_tests();
    const OUTPUT_LIMIT: usize = 4096;

    for (command, args, stdout_should_truncate) in [
        ("/usr/bin/yes", vec!["stdout"], true),
        ("/bin/sh", vec!["-c", "exec /usr/bin/yes stderr >&2"], false),
    ] {
        let result = RuntimeBuilder::default()
            .with_task_group(vec![ExecutionStep::Single(task(command, &args))])
            .with_output_limit(OUTPUT_LIMIT)
            .with_timeout(std::time::Duration::from_secs(2))
            .build()
            .execute()
            .expect("runtime execution failed");
        let RuntimeResult::Success(results) = result else {
            panic!("container setup failed: {result:?}");
        };
        assert_no_task_cgroups();

        let TaskResult::Completed {
            stdout,
            stderr,
            exit_code,
            stats,
        } = single_result(&results[0])
        else {
            panic!("output flood did not produce a result: {:?}", results[0]);
        };

        assert_eq!(*exit_code, 137, "output-limited task was not killed");
        assert!(stdout.len() <= OUTPUT_LIMIT);
        assert!(stderr.len() <= OUTPUT_LIMIT);
        assert_eq!(stats.stdout_truncated, stdout_should_truncate);
        assert_eq!(stats.stderr_truncated, !stdout_should_truncate);
    }
}

#[test]
fn stdin_and_stdout_progress_concurrently_without_pipe_deadlock() {
    let _guard = lock_security_tests();
    let input = "x".repeat(256 * 1024);
    let mut cat_task = task("/bin/cat", &[]);
    cat_task.stdin = Some(input.clone());

    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Single(cat_task)])
        .with_output_limit(input.len())
        .with_timeout(std::time::Duration::from_secs(2))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Completed {
        stdout,
        exit_code,
        stats,
        ..
    } = single_result(&results[0])
    else {
        panic!("cat did not produce a result: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0);
    assert_eq!(stdout, &input);
    assert!(!stats.stdout_truncated);
}

#[test]
fn parallel_large_results_do_not_deadlock_result_transport() {
    let _guard = lock_security_tests();
    const OUTPUT_SIZE: usize = 128 * 1024;

    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Parallel(vec![
            task("/usr/bin/head", &["-c", "131072", "/dev/zero"]),
            task("/usr/bin/head", &["-c", "131072", "/dev/zero"]),
        ])])
        .with_output_limit(OUTPUT_SIZE * 2)
        .with_timeout(std::time::Duration::from_secs(2))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let ExecutionStepResult::Parallel(results) = &results[0] else {
        panic!("expected parallel task results");
    };
    assert_eq!(results.len(), 2);
    for result in results {
        let TaskResult::Completed {
            stdout,
            exit_code,
            stats,
            ..
        } = result
        else {
            panic!("large parallel task failed: {result:?}");
        };
        assert_eq!(*exit_code, 0);
        assert_eq!(stdout.len(), OUTPUT_SIZE);
        assert!(!stats.stdout_truncated);
    }
}

#[test]
fn namespace_init_reaps_orphaned_task_descendants() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task_with_file(
            "/usr/bin/gcc",
            &["orphan_probe.c", "-o", "orphan_probe"],
            "orphan_probe.c",
            ORPHAN_PROBE_SOURCE,
        ),
        task("./orphan_probe", &[]),
        task(
            "/bin/sh",
            &[
                "-c",
                "sleep 0.1; pid=$(cat orphan.pid); test ! -e /proc/$pid",
            ],
        ),
    ]);

    for (index, result) in results.iter().enumerate() {
        let TaskResult::Completed {
            exit_code, stderr, ..
        } = single_result(result)
        else {
            panic!("orphan lifecycle step {index} failed: {result:?}");
        };
        assert_eq!(*exit_code, 0, "orphan lifecycle step {index}: {stderr}");
    }
}

#[test]
fn timeout_kills_the_complete_task_process_tree() {
    let _guard = lock_security_tests();
    let started = std::time::Instant::now();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Single(task(
            "/bin/sh",
            &["-c", "sleep 30 & wait"],
        ))])
        .with_timeout(std::time::Duration::from_millis(150))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Failed { error, .. } = single_result(&results[0]) else {
        panic!(
            "timed-out process tree was reported as completed: {:?}",
            results[0]
        );
    };
    assert!(
        error.contains("timeout"),
        "unexpected timeout error: {error}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "task process tree was not terminated promptly"
    );
}

#[test]
fn pids_cgroup_enforces_the_process_limit() {
    let _guard = lock_security_tests();
    let task_group = vec![
        ExecutionStep::Single(task_with_file(
            "/usr/bin/gcc",
            &["pid_probe.c", "-o", "pid_probe"],
            "pid_probe.c",
            PID_PROBE_SOURCE,
        )),
        ExecutionStep::Single(task("./pid_probe", &[])),
    ];
    let result = RuntimeBuilder::default()
        .with_task_group(task_group)
        .with_cgroup_config(CgroupConfigBuilder::new().with_pids(8).build())
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("PID probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(
        *compile_exit, 0,
        "PID probe did not compile: {compile_stderr}"
    );

    let TaskResult::Completed {
        stdout,
        stderr,
        exit_code,
        stats,
    } = single_result(&results[1])
    else {
        panic!("PID probe did not complete: {:?}", results[1]);
    };
    assert_eq!(*exit_code, 0, "PID probe failed: {stderr}");

    let values: Vec<u32> = stdout
        .split_whitespace()
        .map(|value| value.parse().expect("PID probe emitted a non-number"))
        .collect();
    assert_eq!(values.len(), 2, "unexpected PID probe output: {stdout}");
    assert_eq!(values[1], 11, "fork should fail with EAGAIN: {stdout}");
    assert!(values[0] < 32, "all child processes were created: {stdout}");
    assert!(
        stats.pids_peak <= 8,
        "pids.peak exceeded pids.max: {:?}",
        stats
    );
    assert_eq!(
        stats.pids_peak, 8,
        "the configured PID ceiling was not reached"
    );
}

#[test]
fn memory_cgroup_kills_a_process_that_exceeds_memory_max() {
    let _guard = lock_security_tests();
    const MEMORY_LIMIT: u64 = 64 * 1024 * 1024;

    let task_group = vec![
        ExecutionStep::Single(task_with_file(
            "/usr/bin/gcc",
            &["memory_probe.c", "-o", "memory_probe"],
            "memory_probe.c",
            MEMORY_PROBE_SOURCE,
        )),
        ExecutionStep::Single(task("./memory_probe", &[])),
    ];
    let result = RuntimeBuilder::default()
        .with_task_group(task_group)
        .with_cgroup_config(
            CgroupConfigBuilder::new()
                .with_memory(MEMORY_LIMIT.to_string())
                .build(),
        )
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("memory probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(
        *compile_exit, 0,
        "memory probe did not compile within its cgroup: {compile_stderr}"
    );

    let TaskResult::Completed {
        exit_code, stats, ..
    } = single_result(&results[1])
    else {
        panic!("memory probe did not produce a result: {:?}", results[1]);
    };
    assert_eq!(
        *exit_code, 137,
        "expected the kernel OOM kill signal, got stats: {stats:?}"
    );
    assert!(
        stats.memory_peak_bytes >= MEMORY_LIMIT / 2,
        "memory limit was not approached: {:?}",
        stats
    );
    assert!(
        stats.memory_peak_bytes <= MEMORY_LIMIT + 8 * 1024 * 1024,
        "memory.peak substantially exceeded memory.max: {:?}",
        stats
    );
}
