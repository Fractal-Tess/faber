use faber_config::MountConfig;
use faber_core::Result;
use tracing::info;

pub struct MountManager {
    pub read_only_root: bool,
    pub mount_config: MountConfig,
}

impl MountManager {
    pub fn new(read_only_root: bool, mount_config: MountConfig) -> Self {
        Self {
            read_only_root,
            mount_config,
        }
    }

    pub async fn setup_mounts(&self, work_dir: &str) -> Result<()> {
        info!("Would setup mounts for work_dir: {}", work_dir);
        info!("Mount config: {:?}", self.mount_config);
        // TODO: Implement mount setup using self.mount_config
        Ok(())
    }

    pub async fn cleanup_mounts(&self) -> Result<()> {
        info!("Would cleanup mounts");
        // TODO: Implement mount cleanup
        Ok(())
    }
}
