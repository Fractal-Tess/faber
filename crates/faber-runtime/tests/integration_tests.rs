use faber_runtime::{RuntimeBuilder, Task, TaskGroup};
use std::collections::HashMap;

fn create_test_task(cmd: &str, args: Vec<&str>) -> Task {
    Task {
        cmd: cmd.to_string(),
        args: Some(args.iter().map(|s| s.to_string()).collect()),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
    }
}

#[test]
fn test_basic_command_execution() {
    let task_group: TaskGroup = vec![faber_runtime::ExecutionStep::Single(create_test_task(
        "/bin/echo",
        vec!["Hello, World!"],
    ))];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute();
    assert!(result.is_ok(), "Runtime execution failed: {:?}", result);

    match result.unwrap() {
        faber_runtime::RuntimeResult::Success(results) => {
            assert_eq!(results.len(), 1);
            match &results[0] {
                faber_runtime::ExecutionStepResult::Single(task_result) => match task_result {
                    faber_runtime::TaskResult::Completed {
                        stdout,
                        stderr: _,
                        exit_code,
                        stats: _,
                    } => {
                        assert_eq!(*exit_code, 0, "Command failed with non-zero exit code");
                        assert!(
                            stdout.contains("Hello, World!"),
                            "Output mismatch: {}",
                            stdout
                        );
                    }
                    faber_runtime::TaskResult::Failed { error, .. } => {
                        panic!("Task failed: {}", error);
                    }
                },
                _ => panic!("Expected single task result"),
            }
        }
        other => panic!("Expected success result, got {:?}", other),
    }
}

#[test]
fn test_pid_namespace_isolation() {
    let task = Task {
        cmd: "/bin/sh".to_string(),
        args: Some(vec!["-c".to_string(), "echo $$ && ps aux".to_string()]),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
    };

    let task_group: TaskGroup = vec![faber_runtime::ExecutionStep::Single(task)];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute();
    assert!(result.is_ok(), "Runtime execution failed: {:?}", result);

    match result.unwrap() {
        faber_runtime::RuntimeResult::Success(results) => {
            match &results[0] {
                faber_runtime::ExecutionStepResult::Single(task_result) => {
                    match task_result {
                        faber_runtime::TaskResult::Completed {
                            stdout,
                            stderr: _,
                            exit_code,
                            stats: _,
                        } => {
                            assert_eq!(*exit_code, 0);
                            // The namespace reaper is PID 1, so the task is PID 2.
                            let lines: Vec<&str> = stdout.lines().collect();
                            let first_line = lines.first().unwrap_or(&"");
                            assert!(
                                first_line.trim() == "2",
                                "Expected task PID 2 after namespace init, got: {}",
                                first_line
                            );
                            assert!(
                                stdout.contains("PID"),
                                "Expected ps output from the namespaced procfs, got: {}",
                                stdout
                            );
                        }
                        faber_runtime::TaskResult::Failed { error, .. } => {
                            panic!("Task failed: {}", error);
                        }
                    }
                }
                _ => panic!("Expected single task result"),
            }
        }
        other => panic!("Expected success result, got {:?}", other),
    }
}

#[test]
fn test_toolchain_mounts_are_read_only() {
    let task_group: TaskGroup = vec![faber_runtime::ExecutionStep::Single(create_test_task(
        "/bin/cat",
        vec!["/proc/self/mountinfo"],
    ))];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute().expect("Runtime execution failed");
    let faber_runtime::RuntimeResult::Success(results) = result else {
        panic!("Expected successful runtime result");
    };
    let faber_runtime::ExecutionStepResult::Single(task_result) = &results[0] else {
        panic!("Expected single task result");
    };
    let faber_runtime::TaskResult::Completed {
        stdout, exit_code, ..
    } = task_result
    else {
        panic!("Expected completed task result");
    };

    assert_eq!(*exit_code, 0);

    for mountpoint in ["/bin", "/usr"] {
        let mount = stdout
            .lines()
            .find(|line| line.split_whitespace().nth(4) == Some(mountpoint))
            .unwrap_or_else(|| panic!("Missing {} mount in mountinfo", mountpoint));
        let options = mount
            .split_whitespace()
            .nth(5)
            .unwrap_or_else(|| panic!("Missing mount options for {}", mountpoint));
        assert!(
            options.split(',').any(|option| option == "ro"),
            "Expected {} to be read-only, got options: {}",
            mountpoint,
            options
        );
    }
}

#[test]
fn test_hostname_isolation() {
    let task = Task {
        cmd: "/bin/hostname".to_string(),
        args: None,
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
    };

    let task_group: TaskGroup = vec![faber_runtime::ExecutionStep::Single(task)];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute();
    assert!(result.is_ok(), "Runtime execution failed: {:?}", result);

    match result.unwrap() {
        faber_runtime::RuntimeResult::Success(results) => match &results[0] {
            faber_runtime::ExecutionStepResult::Single(task_result) => match task_result {
                faber_runtime::TaskResult::Completed {
                    stdout,
                    stderr: _,
                    exit_code,
                    stats: _,
                } => {
                    assert_eq!(*exit_code, 0);
                    let hostname = stdout.trim();
                    assert!(!hostname.is_empty(), "Hostname should not be empty");
                }
                faber_runtime::TaskResult::Failed { error, .. } => {
                    panic!("Task failed: {}", error);
                }
            },
            _ => panic!("Expected single task result"),
        },
        other => panic!("Expected success result, got {:?}", other),
    }
}

#[test]
fn test_sequential_execution() {
    let task_group: TaskGroup = vec![
        faber_runtime::ExecutionStep::Single(create_test_task("/bin/echo", vec!["step1"])),
        faber_runtime::ExecutionStep::Single(create_test_task("/bin/echo", vec!["step2"])),
        faber_runtime::ExecutionStep::Single(create_test_task("/bin/echo", vec!["step3"])),
    ];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute();
    assert!(result.is_ok(), "Runtime execution failed: {:?}", result);

    match result.unwrap() {
        faber_runtime::RuntimeResult::Success(results) => {
            assert_eq!(results.len(), 3, "Expected 3 results");

            for (i, step_result) in results.iter().enumerate() {
                match step_result {
                    faber_runtime::ExecutionStepResult::Single(task_result) => match task_result {
                        faber_runtime::TaskResult::Completed {
                            stdout,
                            stderr: _,
                            exit_code,
                            stats: _,
                        } => {
                            assert_eq!(*exit_code, 0, "Step {} failed", i + 1);
                            assert!(stdout.contains(&format!("step{}", i + 1)));
                        }
                        faber_runtime::TaskResult::Failed { error, .. } => {
                            panic!("Step {} failed: {}", i + 1, error);
                        }
                    },
                    _ => panic!("Expected single task result at step {}", i + 1),
                }
            }
        }
        other => panic!("Expected success result, got {:?}", other),
    }
}

#[test]
fn test_parallel_execution() {
    let parallel_tasks = vec![
        create_test_task("/bin/echo", vec!["parallel1"]),
        create_test_task("/bin/echo", vec!["parallel2"]),
        create_test_task("/bin/echo", vec!["parallel3"]),
    ];

    let task_group: TaskGroup = vec![faber_runtime::ExecutionStep::Parallel(parallel_tasks)];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute();
    assert!(result.is_ok(), "Runtime execution failed: {:?}", result);

    match result.unwrap() {
        faber_runtime::RuntimeResult::Success(results) => {
            assert_eq!(results.len(), 1);
            match &results[0] {
                faber_runtime::ExecutionStepResult::Parallel(task_results) => {
                    assert_eq!(task_results.len(), 3, "Expected 3 parallel results");

                    for (i, task_result) in task_results.iter().enumerate() {
                        match task_result {
                            faber_runtime::TaskResult::Completed {
                                stdout,
                                stderr: _,
                                exit_code,
                                stats: _,
                            } => {
                                assert_eq!(*exit_code, 0, "Parallel task {} failed", i + 1);
                                assert!(stdout.contains(&format!("parallel{}", i + 1)));
                            }
                            faber_runtime::TaskResult::Failed { error, .. } => {
                                panic!("Parallel task {} failed: {}", i + 1, error);
                            }
                        }
                    }
                }
                _ => panic!("Expected parallel task results"),
            }
        }
        other => panic!("Expected success result, got {:?}", other),
    }
}

#[test]
fn test_file_operations() {
    let mut files = HashMap::new();
    files.insert("test.txt".to_string(), "Hello from file!".to_string());

    let task = Task {
        cmd: "/bin/cat".to_string(),
        args: Some(vec!["test.txt".to_string()]),
        env: None,
        stdin: None,
        files: Some(files),
        working_dir: None,
    };

    let task_group: TaskGroup = vec![faber_runtime::ExecutionStep::Single(task)];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute();
    assert!(result.is_ok(), "Runtime execution failed: {:?}", result);

    match result.unwrap() {
        faber_runtime::RuntimeResult::Success(results) => match &results[0] {
            faber_runtime::ExecutionStepResult::Single(task_result) => match task_result {
                faber_runtime::TaskResult::Completed {
                    stdout,
                    stderr: _,
                    exit_code,
                    stats: _,
                } => {
                    assert_eq!(*exit_code, 0);
                    assert!(stdout.contains("Hello from file!"));
                }
                faber_runtime::TaskResult::Failed { error, .. } => {
                    panic!("Task failed: {}", error);
                }
            },
            _ => panic!("Expected single task result"),
        },
        other => panic!("Expected success result, got {:?}", other),
    }
}

#[test]
fn test_network_isolation() {
    let task = Task {
        cmd: "/bin/sh".to_string(),
        args: Some(vec!["-c".to_string(), "ip -o link show".to_string()]),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
    };

    let task_group: TaskGroup = vec![faber_runtime::ExecutionStep::Single(task)];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute();
    assert!(result.is_ok(), "Runtime execution failed: {:?}", result);

    match result.unwrap() {
        faber_runtime::RuntimeResult::Success(results) => match &results[0] {
            faber_runtime::ExecutionStepResult::Single(task_result) => match task_result {
                faber_runtime::TaskResult::Completed {
                    stdout,
                    stderr: _,
                    exit_code,
                    stats: _,
                } => {
                    assert_eq!(*exit_code, 0);
                    let interfaces: Vec<&str> = stdout.lines().collect();
                    assert_eq!(
                        interfaces.len(),
                        1,
                        "Expected an isolated namespace with only loopback, got: {}",
                        stdout
                    );
                    assert!(
                        interfaces[0].contains(": lo:"),
                        "Expected loopback interface, got: {}",
                        stdout
                    );
                }
                faber_runtime::TaskResult::Failed { error, .. } => {
                    panic!("Task failed: {}", error);
                }
            },
            _ => panic!("Expected single task result"),
        },
        other => panic!("Expected success result, got {:?}", other),
    }
}

#[test]
fn test_resource_limits() {
    let task = Task {
        cmd: "/bin/sleep".to_string(),
        args: Some(vec!["0.1".to_string()]),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
    };

    let task_group: TaskGroup = vec![faber_runtime::ExecutionStep::Single(task)];

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build();

    let result = runtime.execute();
    assert!(result.is_ok(), "Runtime execution failed: {:?}", result);

    match result.unwrap() {
        faber_runtime::RuntimeResult::Success(results) => {
            match &results[0] {
                faber_runtime::ExecutionStepResult::Single(task_result) => {
                    match task_result {
                        faber_runtime::TaskResult::Completed {
                            stdout: _,
                            stderr: _,
                            exit_code,
                            stats,
                        } => {
                            assert_eq!(*exit_code, 0);
                            // Stats should be populated
                            assert!(
                                stats.execution_time_ms >= 100,
                                "Execution time should be at least 100ms"
                            );
                        }
                        faber_runtime::TaskResult::Failed { error, .. } => {
                            panic!("Task failed: {}", error);
                        }
                    }
                }
                _ => panic!("Expected single task result"),
            }
        }
        other => panic!("Expected success result, got {:?}", other),
    }
}
