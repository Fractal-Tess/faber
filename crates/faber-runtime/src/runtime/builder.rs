use std::time::Duration;

use crate::{
    Runtime,
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

    pub fn build(self) -> Runtime {
        Runtime {
            task_group: self.task_group,
            container: self.container,
            cgroup: self.cgroup,
            timeout: self.timeout,
            cpu_time_limit: self.cpu_time_limit,
            output_limit: self.output_limit,
        }
    }
}
