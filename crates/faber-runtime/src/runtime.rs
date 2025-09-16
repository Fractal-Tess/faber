use std::{
    io::{PipeWriter, Write},
    os::fd::IntoRawFd,
    process::exit,
};

use nix::{
    sys::wait::waitpid,
    unistd::{ForkResult, fork},
};

use crate::{
    ExecutionStep, ExecutionStepResult, TaskGroup, TaskResult, TaskResultStats,
    container::Container,
    prelude::*,
    result::TaskGroupResult,
    utils::{close_fd, mk_pipe},
};

pub struct Runtime {
    task_group: TaskGroup,
    container: Container,
}

impl Runtime {
    pub fn new(task_group: TaskGroup) -> Self {
        let container = Container::default();

        Self {
            task_group,
            container,
        }
    }

    pub fn execute(&self) -> Result<TaskGroupResult> {
        let (reader, writer) = mk_pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                close_fd(reader.into_raw_fd())?;

                self.execution_child(writer)?;
                exit(0);
            }
            Ok(ForkResult::Parent { child }) => {
                close_fd(writer.into_raw_fd())?;

                waitpid(child, None).unwrap();

                let results: TaskGroupResult = serde_json::from_reader(reader).unwrap();

                Ok(results)
            }

            Err(e) => Err(FaberError::Fork { e }),
        }
    }

    fn execution_child(&self, mut writer: PipeWriter) -> Result<()> {
        self.container.setup()?;

        let mut task_group_result: TaskGroupResult =
            vec![ExecutionStepResult::Single(TaskResult::Completed {
                stdout: "Hello, world!".to_string(),
                stderr: "".to_string(),
                exit_code: 0,
                stats: TaskResultStats::default(),
            })];

        serde_json::to_writer(writer, &task_group_result).unwrap();

        self.container.cleanup()?;

        Ok(())
    }
}
