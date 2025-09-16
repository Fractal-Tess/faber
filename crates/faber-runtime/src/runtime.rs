use crate::{TaskGroup, container::Container};

pub struct Runtime {
    task_group: TaskGroup,
}

impl Runtime {
    fn new(task_group: TaskGroup) -> Self {
        let container = Container::new();

        Self { task_group }
    }
}
