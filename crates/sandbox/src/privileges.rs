use faber_core::Result;
use tracing::info;

pub struct PrivilegeManager {
    pub uid: u32,
    pub gid: u32,
}

impl PrivilegeManager {
    pub fn new(uid: u32, gid: u32) -> Self {
        Self { uid, gid }
    }

    pub async fn drop_privileges(&self) -> Result<()> {
        info!(
            "Would drop privileges to uid: {}, gid: {}",
            self.uid, self.gid
        );
        // TODO: Implement privilege dropping
        Ok(())
    }

    pub async fn restore_privileges(&self) -> Result<()> {
        info!("Would restore privileges");
        // TODO: Implement privilege restoration
        Ok(())
    }
}
