use axum::{Json, extract::State};
use faber_api::{AppState, handlers::execute};
use faber_runtime::{ExecutionStep, Task};
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
