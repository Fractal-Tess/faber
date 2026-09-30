//! Runs in its own test binary: `Runtime::shutdown` is process-wide and
//! permanent, so it must not share a process with other runtime tests.

use faber_runtime::{ExecutionStep, FaberError, Runtime, RuntimeBuilder, Task};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn faber_cgroup_path() -> PathBuf {
    let membership = std::fs::read_to_string("/proc/self/cgroup")
        .expect("failed to read the test process cgroup membership");
    let relative_path = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .expect("test process is not in a cgroup v2 hierarchy");
    PathBuf::from("/sys/fs/cgroup")
        .join(relative_path.trim_start_matches('/'))
        .ancestors()
        .map(|ancestor| ancestor.join("faber"))
        .find(|candidate| candidate.is_dir())
        .expect("failed to locate the Faber cgroup")
}

fn sandbox_cgroups() -> Vec<PathBuf> {
    std::fs::read_dir(faber_cgroup_path())
        .expect("failed to inspect the Faber cgroup")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("req-") || name.starts_with("task-")
            })
        })
        .collect()
}

fn sleep_task() -> Task {
    Task {
        cmd: "/bin/sleep".to_string(),
        args: Some(vec!["4".to_string()]),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
        sandbox_profile: None,
    }
}

#[test]
fn shutdown_stops_later_steps_and_refuses_new_executions() {
    let running = std::thread::spawn(|| {
        RuntimeBuilder::default()
            .with_task_group(
                (0..3)
                    .map(|_| ExecutionStep::Single(sleep_task()))
                    .collect(),
            )
            .with_timeout(Duration::from_secs(5))
            .build()
            .execute()
    });

    for _ in 0..200 {
        if !sandbox_cgroups().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_secs(1));

    let shutdown_started = Instant::now();
    Runtime::shutdown().expect("shutdown failed");
    let result = running.join().expect("runtime panicked");
    let stopped_after = shutdown_started.elapsed();

    assert!(
        matches!(result, Err(FaberError::ShuttingDown)),
        "running execution was not stopped: {result:?}"
    );
    assert!(
        stopped_after < Duration::from_millis(1500),
        "execution kept running {stopped_after:?} after shutdown"
    );
    assert!(
        sandbox_cgroups().is_empty(),
        "sandboxes survived shutdown: {:?}",
        sandbox_cgroups()
    );

    let refused = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Single(sleep_task())])
        .build()
        .execute();
    assert!(matches!(refused, Err(FaberError::ShuttingDown)));
}
