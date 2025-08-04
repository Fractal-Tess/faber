use faber_core::Result;
use tracing::info;

pub struct CgroupsManager {
    pub prefix: String,
    pub base_path: Option<String>,
}

impl CgroupsManager {
    pub fn new(prefix: String, base_path: Option<String>) -> Self {
        Self { prefix, base_path }
    }

    pub async fn create_cgroup(&self, name: &str) -> Result<()> {
        info!("Would create cgroup: {}/{}", self.prefix, name);
        // TODO: Implement actual cgroup creation
        Ok(())
    }

    pub async fn set_memory_limit(&self, name: &str, limit: u64) -> Result<()> {
        info!(
            "Would set memory limit for {}/{}: {} bytes",
            self.prefix, name, limit
        );
        // TODO: Implement memory limit setting
        Ok(())
    }

    pub async fn set_cpu_limit(&self, name: &str, limit: u32) -> Result<()> {
        info!(
            "Would set CPU limit for {}/{}: {}%",
            self.prefix, name, limit
        );
        // TODO: Implement CPU limit setting
        Ok(())
    }

    pub async fn cleanup_cgroup(&self, name: &str) -> Result<()> {
        info!("Would cleanup cgroup: {}/{}", self.prefix, name);
        // TODO: Implement cgroup cleanup
        Ok(())
    }
}
