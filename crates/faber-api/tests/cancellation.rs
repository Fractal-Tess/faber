use axum::{Json, extract::State};
use faber_api::{AppState, ExecutionLimits, handlers::execute};
use faber_runtime::{ExecutionStep, Task};
use faber_runtime::{ExecutionStepResult, TaskOutcome, TaskResult};
use faber_store::{StoreConfig, create_store};
use std::{path::PathBuf, time::Duration};

fn faber_cgroup_path() -> Option<PathBuf> {
    let membership = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let relative_path = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))?;
    let own_path = PathBuf::from("/sys/fs/cgroup").join(relative_path.trim_start_matches('/'));

    own_path
        .ancestors()
        .map(|ancestor| ancestor.join("faber"))
        .find(|candidate| candidate.is_dir())
}

#[tokio::test(flavor = "current_thread")]
async fn api_memory_limit_reports_out_of_memory() {
    let state = AppState::new(
        "test-key".to_string(),
        false,
        create_store(StoreConfig::default()),
        ExecutionLimits {
            memory_max: "8M".to_string(),
            ..ExecutionLimits::default()
        },
    );
    let task = Task {
        cmd: "/bin/dd".to_string(),
        args: Some(vec![
            "if=/dev/zero".to_string(),
            "of=/faber/oom.bin".to_string(),
            "bs=1M".to_string(),
            "count=64".to_string(),
        ]),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
        sandbox_profile: None,
    };

    let Json(results) = execute(State(state), Json(vec![ExecutionStep::Single(task)]))
        .await
        .expect("API execution failed");
    let ExecutionStepResult::Single(TaskResult::Completed { stats, .. }) = &results[0] else {
        panic!("unexpected task result: {:?}", results[0]);
    };
    assert_eq!(stats.outcome, TaskOutcome::OutOfMemory);
}

#[tokio::test(flavor = "current_thread")]
async fn api_rejects_parallel_fanout_before_execution() {
    let state = AppState::new(
        "test-key".to_string(),
        false,
        create_store(StoreConfig::default()),
        ExecutionLimits {
            max_steps: 1,
            max_parallel_tasks: 1,
            ..ExecutionLimits::default()
        },
    );
    let task = Task {
        cmd: "/bin/true".to_string(),
        args: None,
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
        sandbox_profile: None,
    };

    let response = execute(
        State(state),
        Json(vec![ExecutionStep::Parallel(vec![task.clone(), task])]),
    )
    .await;
    assert!(matches!(
        response,
        Err((axum::http::StatusCode::UNPROCESSABLE_ENTITY, _))
    ));
    assert!(task_cgroups().is_empty());
}

fn task_cgroups() -> Vec<PathBuf> {
    let Some(faber_path) = faber_cgroup_path() else {
        return Vec::new();
    };
    std::fs::read_dir(faber_path)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("task-"))
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn aborting_an_api_request_still_cleans_the_detached_runtime() {
    let state = AppState::new(
        "test-key".to_string(),
        false,
        create_store(StoreConfig::default()),
        ExecutionLimits::default(),
    );
    let task = Task {
        cmd: "/bin/sh".to_string(),
        args: Some(vec!["-c".to_string(), "sleep 30 & wait".to_string()]),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
        sandbox_profile: None,
    };

    let request = tokio::spawn(execute(
        State(state),
        Json(vec![ExecutionStep::Single(task)]),
    ));

    let mut observed_execution = false;
    for _ in 0..100 {
        if !task_cgroups().is_empty() {
            observed_execution = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(observed_execution, "request never created a task cgroup");

    request.abort();
    assert!(
        request
            .await
            .expect_err("aborted request completed")
            .is_cancelled()
    );

    for _ in 0..400 {
        if task_cgroups().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!(
        "detached runtime leaked task cgroups after its wall timeout: {:?}",
        task_cgroups()
    );
}
