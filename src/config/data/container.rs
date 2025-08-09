use serde::{Deserialize, Deserializer};

use faber::Mount;
use nix::mount::MsFlags;

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerConfig {
    pub filesystem: ContainerFilesystemConfig,
    pub cgroups: Option<CgroupsConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerFilesystemConfig {
    pub base_dir: String,
    pub work_dir: String,
    #[serde(default, deserialize_with = "deserialize_filesystem_mounts")]
    pub mounts: Vec<Mount>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemMount {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub readonly: bool,
}

fn deserialize_filesystem_mounts<'de, D>(deserializer: D) -> Result<Vec<Mount>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: Vec<FilesystemMount> = Vec::<FilesystemMount>::deserialize(deserializer)?;
    let mut mounts: Vec<Mount> = Vec::with_capacity(raw.len());
    for m in raw {
        let mut flags = vec![MsFlags::MS_BIND, MsFlags::MS_REC];
        if m.readonly {
            flags.push(MsFlags::MS_RDONLY);
        }
        mounts.push(Mount {
            source: m.source,
            target: m.target,
            flags,
            options: vec![],
            data: None,
        });
    }
    Ok(mounts)
}

#[derive(Debug, Clone, Deserialize)]
pub struct CgroupsConfig {
    pub pids_max: Option<String>,
    pub memory_max: Option<String>,
    pub cpu_max: Option<String>,
}
