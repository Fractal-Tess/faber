#[derive(Debug, Clone)]
pub struct CgroupConfig {
    pub(crate) cpu_max: String,
    pub(crate) memory_max: String,
    pub(crate) pids_max: u32,
}

impl Default for CgroupConfig {
    fn default() -> Self {
        Self {
            cpu_max: "50000 100000".to_string(),
            // Use "max" to inherit parent's memory limit without additional constraints
            // This prevents ENOMEM when running inside Docker with memory limits
            memory_max: "max".to_string(),
            pids_max: 64,
        }
    }
}
