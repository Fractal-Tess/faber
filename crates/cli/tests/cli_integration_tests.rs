use std::fs;
use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn test_cli_binary_help() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("faber"));
    assert!(stdout.contains("A secure sandboxed task execution service"));
    assert!(stdout.contains("serve"));
    assert!(stdout.contains("validate"));
    assert!(stdout.contains("config"));
}

#[test]
fn test_cli_binary_version() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "--version"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("faber"));
    assert!(stdout.contains("0.1.0"));
}

#[test]
fn test_cli_validate_command() {
    // Create a temporary config file
    let config_content = r#"
[api]
host = "127.0.0.1"
port = 8080

[api.cors]
enable_cors = false
cors_allowed_origins = "*"
cors_allowed_methods = "GET,POST,OPTIONS"
cors_allowed_headers = "*"
cors_allow_credentials = false

[api.request]
max_request_size_kb = 10240

[api.auth]
enable = "env:FABER_AUTH_ENABLE|false"
secret_key = "env:FABER_AUTH_SECRET_KEY"

[api.endpoints]
health_endpoint = "/health"
execute_endpoint = "/execute-tasks"

[sandbox.resource_limits]
memory_limit_kb = 524288
cpu_time_limit_ms = 10000
max_cpu_cores = 1
wall_time_limit_ms = 30000
max_processes = 50
max_fds = 256
stack_limit_kb = 4
data_segment_limit_kb = 256
address_space_limit_kb = 1024
cpu_rate_limit_percent = 50
io_read_limit_kb_s = 10
io_write_limit_kb_s = 10

[sandbox.cgroups]
enabled = true
prefix = "faber"
version = "v2"
enable_cpu_rate_limit = true
enable_memory_limit = true
enable_process_limit = true

[sandbox.filesystem]
readonly = true
tmpfs_size_mb = 100

[sandbox.filesystem.mounts]
readable = { "src" = ["/tmp/test"] }
writable = { "output" = ["/tmp/output"] }
tmpfs = { "temp" = ["/tmp/temp"] }

[sandbox.security]
default_security_level = "standard"

[sandbox.security.namespaces]
pid = false
mount = true
network = true
ipc = true
uts = true
user = true
time = false
cgroup = true

[sandbox.security.seccomp]
enabled = true
default_action = "SCMP_ACT_ERRNO"
architectures = ["SCMP_ARCH_X86_64", "SCMP_ARCH_X86", "SCMP_ARCH_AARCH64"]

[sandbox.security.seccomp.syscalls]
allowed = ["read", "write", "open", "close", "exit", "exit_group"]
disallowed = []
"#;

    let temp_file = NamedTempFile::new().unwrap();
    fs::write(&temp_file, config_content).unwrap();

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "faber",
            "--",
            "validate",
            temp_file.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Configuration is valid"));
}

#[test]
fn test_cli_validate_invalid_config() {
    let temp_file = NamedTempFile::new().unwrap();
    fs::write(&temp_file, "invalid toml content").unwrap();

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "faber",
            "--",
            "validate",
            temp_file.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Configuration validation failed"));
}

#[test]
fn test_cli_config_default_command() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "config", "--default"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("api"));
    assert!(stdout.contains("sandbox"));
}

#[test]
fn test_cli_config_from_file() {
    // Create a temporary config file
    let config_content = r#"
[api]
host = "127.0.0.1"
port = 8080

[api.cors]
enable_cors = false
cors_allowed_origins = "*"
cors_allowed_methods = "GET,POST,OPTIONS"
cors_allowed_headers = "*"
cors_allow_credentials = false

[api.request]
max_request_size_kb = 10240

[api.auth]
enable = "env:FABER_AUTH_ENABLE|false"
secret_key = "env:FABER_AUTH_SECRET_KEY"

[api.endpoints]
health_endpoint = "/health"
execute_endpoint = "/execute-tasks"

[sandbox.resource_limits]
memory_limit_kb = 524288
cpu_time_limit_ms = 10000
max_cpu_cores = 1
wall_time_limit_ms = 30000
max_processes = 50
max_fds = 256
stack_limit_kb = 4
data_segment_limit_kb = 256
address_space_limit_kb = 1024
cpu_rate_limit_percent = 50
io_read_limit_kb_s = 10
io_write_limit_kb_s = 10

[sandbox.cgroups]
enabled = true
prefix = "faber"
version = "v2"
enable_cpu_rate_limit = true
enable_memory_limit = true
enable_process_limit = true

[sandbox.filesystem]
readonly = true
tmpfs_size_mb = 100

[sandbox.filesystem.mounts]
readable = { "src" = ["/tmp/test"] }
writable = { "output" = ["/tmp/output"] }
tmpfs = { "temp" = ["/tmp/temp"] }

[sandbox.security]
default_security_level = "standard"

[sandbox.security.namespaces]
pid = false
mount = true
network = true
ipc = true
uts = true
user = true
time = false
cgroup = true

[sandbox.security.seccomp]
enabled = true
default_action = "SCMP_ACT_ERRNO"
architectures = ["SCMP_ARCH_X86_64", "SCMP_ARCH_X86", "SCMP_ARCH_AARCH64"]

[sandbox.security.seccomp.syscalls]
allowed = ["read", "write", "open", "close", "exit", "exit_group"]
disallowed = []
"#;

    let temp_file = NamedTempFile::new().unwrap();
    fs::write(&temp_file, config_content).unwrap();

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "faber",
            "--",
            "--config",
            temp_file.path().to_str().unwrap(),
            "config",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("127.0.0.1"));
    assert!(stdout.contains("8080"));
}

#[test]
fn test_cli_with_log_level() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "faber",
            "--",
            "--log-level",
            "debug",
            "--help",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("faber"));
}

#[test]
fn test_cli_with_debug_flag() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "--debug", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("faber"));
}

#[test]
fn test_cli_serve_command_help() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "serve", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("serve"));
    assert!(stdout.contains("graceful-shutdown"));
}

#[test]
fn test_cli_validate_command_help() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "validate", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("validate"));
    assert!(stdout.contains("config"));
}

#[test]
fn test_cli_config_command_help() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "config", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("config"));
    assert!(stdout.contains("default"));
}

#[test]
fn test_cli_no_command_shows_help() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("faber"));
    assert!(stdout.contains("A secure sandboxed task execution service"));
}

#[test]
fn test_cli_invalid_option() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "--invalid-option"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error"));
}

#[test]
fn test_cli_invalid_subcommand() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "faber", "--", "invalid-command"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error"));
}
