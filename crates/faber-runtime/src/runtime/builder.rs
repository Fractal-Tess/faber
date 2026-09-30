use std::time::Duration;

use crate::{
    CancellationToken, Runtime,
    cgroup::{Cgroup, CgroupConfig},
    container::{Container, ContainerConfig},
    task::TaskGroup,
};

pub struct RuntimeBuilder {
    task_group: TaskGroup,
    container: Container,
    cgroup: Cgroup,
    timeout: Duration,
    cpu_time_limit: Duration,
    output_limit: usize,
    request_output_limit: usize,
    overall_timeout: Duration,
    cancellation: CancellationToken,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self {
            task_group: vec![],
            container: Container::default(),
            cgroup: Cgroup::default(),
            timeout: Duration::from_secs(5),
            cpu_time_limit: Duration::from_secs(5),
            output_limit: 1024 * 1024,
            request_output_limit: usize::MAX,
            overall_timeout: Duration::from_secs(300),
            cancellation: CancellationToken::new(),
        }
    }
}

impl RuntimeBuilder {
    pub fn with_task_group(mut self, task_group: TaskGroup) -> Self {
        self.task_group = task_group;
        self
    }

    pub fn with_cgroup_config(mut self, cgroup_config: CgroupConfig) -> Self {
        self.cgroup = Cgroup::new(cgroup_config);
        self
    }

    pub fn with_container_config(mut self, container_config: ContainerConfig) -> Self {
        self.container = Container::new(container_config);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_cpu_time_limit(mut self, cpu_time_limit: Duration) -> Self {
        self.cpu_time_limit = cpu_time_limit;
        self
    }

    pub fn with_output_limit(mut self, output_limit: usize) -> Self {
        self.output_limit = output_limit;
        self
    }

    /// Output bytes (stdout plus stderr) kept across every task of the
    /// request. Once spent, later tasks get no budget and report
    /// `output_limit` as soon as they write.
    pub fn with_request_output_limit(mut self, request_output_limit: usize) -> Self {
        self.request_output_limit = request_output_limit;
        self
    }

    pub fn with_overall_timeout(mut self, overall_timeout: Duration) -> Self {
        self.overall_timeout = overall_timeout;
        self
    }

    /// Cancel the execution through this token, e.g. when its caller goes away.
    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = cancellation;
        self
    }

    pub fn build(self) -> Runtime {
        Runtime {
            task_group: self.task_group,
            container: self.container,
            cgroup: self.cgroup,
            timeout: self.timeout,
            cpu_time_limit: self.cpu_time_limit,
            output_limit: self.output_limit,
            request_output_limit: self.request_output_limit,
            overall_timeout: self.overall_timeout,
            cancellation: self.cancellation,
        }
    }
}
