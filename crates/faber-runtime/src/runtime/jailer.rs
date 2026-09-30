//! The jailer: the root process that builds one request's sandbox and
//! supervises its tasks.
//!
//! It is this same executable, started again with [`JAILER_ENV`] set, and it
//! takes over before `main` runs. The embedding process therefore never forks
//! sandbox code out of a multithreaded address space, and the jailer holds
//! none of the embedder's memory, environment or descriptors: only the job it
//! reads from stdin.

use std::{
    io::{PipeReader, Read, Write},
    os::{
        fd::{FromRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use nix::{libc, unistd::Pid};
use serde::{Deserialize, Serialize};

use super::core::Runtime;
use crate::{
    CancellationToken,
    cgroup::{Cgroup, CgroupConfig},
    container::{Container, ContainerConfig},
    prelude::*,
    result::RuntimeResult,
    task::TaskGroup,
};

const JAILER_ENV: &str = "__FABER_JAILER";

/// Everything the jailer needs to run one request.
#[derive(Serialize, Deserialize)]
pub(crate) struct JailerJob {
    pub(crate) task_group: TaskGroup,
    pub(crate) container: ContainerConfig,
    pub(crate) cgroup: CgroupConfig,
    pub(crate) request_cgroup_path: PathBuf,
    /// Leaf of the request cgroup the jailer and its supervisors run in.
    pub(crate) supervisor_cgroup_path: PathBuf,
    pub(crate) timeout: Duration,
    pub(crate) cpu_time_limit: Duration,
    pub(crate) output_limit: usize,
    pub(crate) request_output_limit: usize,
    /// Time left until the request's overall deadline.
    pub(crate) remaining: Duration,
    /// Host UID and GID the tasks run as.
    pub(crate) identity: u32,
    pub(crate) controller_pid: u32,
}

// Runs before `main` in every executable that links this crate, including
// test harnesses, so that any of them can serve as its own jailer.
#[used]
#[unsafe(link_section = ".init_array")]
static JAILER_ENTRY: extern "C" fn() = {
    extern "C" fn entry() {
        if std::env::var_os(JAILER_ENV).is_some() {
            run();
        }
    }
    entry
};

/// Start a jailer for `job` in its own process group. Returns its PID and the
/// pipe its [`RuntimeResult`] arrives on.
pub(crate) fn spawn(job: &JailerJob) -> Result<(Pid, PipeReader)> {
    let failed = |stage: &str, error: &dyn std::fmt::Display| FaberError::Generic {
        message: format!("Failed to {stage} the jailer: {error}"),
    };

    let payload = serde_json::to_vec(job).map_err(|error| failed("describe the job to", &error))?;
    let mut child = Command::new("/proc/self/exe")
        .arg0("faber-jailer")
        .env_clear()
        .env(JAILER_ENV, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .process_group(0)
        .spawn()
        .map_err(|error| failed("start", &error))?;
    let pid = Pid::from_raw(child.id() as i32);

    let handed_over = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("no stdin pipe"))
        .and_then(|mut stdin| stdin.write_all(&payload));
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("no stdout pipe"));
    match (handed_over, stdout) {
        (Ok(()), Ok(stdout)) => Ok((pid, PipeReader::from(OwnedFd::from(stdout)))),
        (Err(error), _) | (_, Err(error)) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(failed("hand the job to", &error))
        }
    }
}

fn run() -> ! {
    // Nothing but the standard streams is meant to arrive here.
    unsafe { libc::syscall(libc::SYS_close_range, 3_u32, u32::MAX, 0_u32) };
    Runtime::die_with_parent();

    let mut stdout = unsafe { std::fs::File::from_raw_fd(libc::STDOUT_FILENO) };
    let result = read_job().map_or_else(
        |error| RuntimeResult::ContainerSetupFailed {
            error: format!("The jailer could not read its job: {error}"),
        },
        |job| {
            // The controller may have died before die_with_parent took effect.
            if nix::unistd::getppid().as_raw() as u32 != job.controller_pid {
                Runtime::child_exit(125);
            }
            if let Err(error) = join_supervisor_cgroup(&job.supervisor_cgroup_path) {
                return RuntimeResult::ContainerSetupFailed {
                    error: format!("The jailer could not join the request cgroup: {error}"),
                };
            }
            let deadline = Instant::now() + job.remaining;
            let runtime = Runtime {
                task_group: job.task_group,
                container: Container::new(job.container),
                cgroup: Cgroup::new(job.cgroup),
                timeout: job.timeout,
                cpu_time_limit: job.cpu_time_limit,
                output_limit: job.output_limit,
                request_output_limit: job.request_output_limit,
                overall_timeout: job.remaining,
                cancellation: CancellationToken::new(),
            };
            runtime.execution_child(&job.request_cgroup_path, deadline, job.identity)
        },
    );
    Runtime::write_child_result(&mut stdout, &result);
    Runtime::child_exit(0)
}

/// Charge the jailer and everything it forks to the request, and make them
/// the last choice of the OOM killer within it: a task over the limit is
/// killed before the process supervising it.
fn join_supervisor_cgroup(cgroup: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(cgroup.join("cgroup.procs"), std::process::id().to_string())?;
    std::fs::write("/proc/self/oom_score_adj", "-999")
}

fn read_job() -> std::io::Result<JailerJob> {
    let mut payload = Vec::new();
    std::io::stdin().lock().read_to_end(&mut payload)?;
    serde_json::from_slice(&payload).map_err(std::io::Error::other)
}
