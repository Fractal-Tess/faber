use faber_core::Task;

pub struct ExecutorConfig {
    pub tasks: Vec<Task>,
    pub container: Container,
}
