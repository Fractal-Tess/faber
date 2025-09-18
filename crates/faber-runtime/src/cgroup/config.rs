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
            memory_max: "128M".to_string(),
            pids_max: 64,
        }
    }
}
