use faber_runtime::{
    CgroupConfigBuilder, ExecutionStep, ExecutionStepResult, RuntimeBuilder, RuntimeResult, Task,
    TaskResult,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

static SECURITY_TEST_LOCK: Mutex<()> = Mutex::new(());

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
