use axum::{Json, body::Body, extract::State, http::Request};
use faber_api::{AppState, ExecutionLimits, build_router, handlers::execute};
use faber_runtime::{ExecutionStep, Task};
use faber_runtime::{ExecutionStepResult, TaskOutcome, TaskResult};
use faber_store::{StoreConfig, create_store};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tower::ServiceExt;

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
    assert_eq!(stats.outcome, TaskOutcome::OutOfMemory, "{:?}", results[0]);
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
        Err(error) if error.status() == axum::http::StatusCode::UNPROCESSABLE_ENTITY
    ));
    assert!(sandbox_cgroups().is_empty());
}

/// Request cgroups (and any legacy top-level task cgroups) under Faber.
fn sandbox_cgroups() -> Vec<PathBuf> {
    let Some(faber_path) = faber_cgroup_path() else {
        return Vec::new();
    };
    std::fs::read_dir(faber_path)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("req-") || name.starts_with("task-")
            })
        })
        .collect()
}

/// Task cgroups that currently exist beneath any request cgroup.
fn task_cgroups() -> Vec<PathBuf> {
    sandbox_cgroups()
        .into_iter()
        .flat_map(|request| {
            std::fs::read_dir(&request)
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| {
                    path.file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("task-"))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn aborting_an_api_request_cancels_its_sandbox() {
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

    let aborted_at = Instant::now();
    request.abort();
    assert!(
        request
            .await
            .expect_err("aborted request completed")
            .is_cancelled()
    );

    // The wall timeout is 5 s; cancellation must tear the sandbox down well
    // before that.
    while aborted_at.elapsed() < Duration::from_millis(1500) {
        if sandbox_cgroups().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!(
        "cancelled request left its sandbox running: {:?}",
        sandbox_cgroups()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn disconnecting_clients_cannot_exceed_the_concurrency_limit() {
    const LIMIT: usize = 2;
    let router = build_router(
        "test-key".to_string(),
        false,
        create_store(StoreConfig::default()),
        ExecutionLimits {
            max_concurrency: LIMIT,
            ..ExecutionLimits::default()
        },
    );

    // Every 100 ms a client submits a 4 s task and disconnects after 300 ms.
    let mut clients = Vec::new();
    let mut peak = 0;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(2) {
        let request = Request::post("/execute")
            .header("Authorization", "Bearer test-key")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"[{"cmd":"/bin/sleep","args":["4"]}]"#))
            .unwrap();
        let router = router.clone();
        clients.push(tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(300), router.oneshot(request)).await;
        }));
        for _ in 0..5 {
            peak = peak.max(task_cgroups().len());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    for client in clients {
        client.await.expect("client task panicked");
    }

    assert!(
        peak <= LIMIT,
        "{peak} sandboxes ran at once with a limit of {LIMIT}"
    );
    for _ in 0..100 {
        if sandbox_cgroups().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("sandboxes leaked: {:?}", sandbox_cgroups());
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_steps_reserve_one_slot_per_task() {
    let router = build_router(
        "test-key".to_string(),
        false,
        create_store(StoreConfig::default()),
        ExecutionLimits {
            max_concurrency: 4,
            max_parallel_tasks: 4,
            ..ExecutionLimits::default()
        },
    );
    let post = |body: &'static str| {
        Request::post("/execute")
            .header("Authorization", "Bearer test-key")
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap()
    };

    let wide = tokio::spawn(router.clone().oneshot(post(
        r#"[[{"cmd":"/bin/sleep","args":["1"]},{"cmd":"/bin/sleep","args":["1"]},{"cmd":"/bin/sleep","args":["1"]},{"cmd":"/bin/sleep","args":["1"]}]]"#,
    )));
    for _ in 0..100 {
        if !task_cgroups().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let narrow = router
        .clone()
        .oneshot(post(r#"[{"cmd":"/bin/true"}]"#))
        .await
        .unwrap();
    assert_eq!(
        narrow.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "a 4-wide step must hold all 4 slots"
    );

    let wide = wide.await.unwrap().unwrap();
    assert_eq!(wide.status(), axum::http::StatusCode::OK);
    let narrow = router
        .oneshot(post(r#"[{"cmd":"/bin/true"}]"#))
        .await
        .unwrap();
    assert_eq!(narrow.status(), axum::http::StatusCode::OK);
}
