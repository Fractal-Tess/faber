use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};

use super::MountsConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerConfig {
    pub cgroups: ContainerCgroupsConfig,
    pub syscall_blocklist: SyscallBlocklistConfig,
    pub filesystem: ContainerFilesystemConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerCgroupsConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyscallBlocklistConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerFilesystemConfig {
    pub base_dir: String,
    #[serde(deserialize_with = "deserialize_tempfs_dir")]
    pub work_dir: TempfsDir,
    #[serde(deserialize_with = "deserialize_tempfs_dir")]
    pub tmp_dir: TempfsDir,
    pub mounts: MountsConfig,
}

#[derive(Debug, Clone)]
pub struct TempfsDir {
    pub target: String,
    pub options: String,
}

fn deserialize_tempfs_dir<'de, D>(deserializer: D) -> Result<TempfsDir, D::Error>
where
    D: Deserializer<'de>,
{
    let values: Vec<String> = Vec::<String>::deserialize(deserializer)?;
    if values.len() != 2 {
        return Err(DeError::custom(format!(
            "tempfs dir must have exactly 2 elements [target, options], got {}",
            values.len()
        )));
    }

    Ok(TempfsDir {
        target: values[0].clone().trim_start_matches('/').to_string(),
        options: values[1].clone(),
    })
}
