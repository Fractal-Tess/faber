use serde::{Deserialize, Deserializer};

use faber::Mount;
use nix::mount::MsFlags;

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerConfig {
    pub filesystem: ContainerFilesystemConfig,
    pub cgroup: CgroupConfig,
    pub runtime: ContainerRuntimeConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerFilesystemConfig {
    pub base_dir: String,
    pub work_dir: String,
    #[serde(default = "default_tmp_size")]
    pub tmp_size: String,
    #[serde(default = "default_workdir_size")]
    pub workdir_size: String,
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

fn default_tmp_size() -> String {
    "128M".to_string()
}

fn default_workdir_size() -> String {
    "256M".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct CgroupConfig {
    #[serde(default = "default_enabled_true")]
    pub enabled: bool,
    pub pids_max: Option<u64>,
    pub memory_max: Option<String>,
    pub cpu_max: Option<String>,
}

fn default_enabled_true() -> bool {
    true
}

impl From<CgroupConfig> for faber::CgroupConfig {
    fn from(cfg: CgroupConfig) -> Self {
        faber::CgroupConfig {
            enabled: cfg.enabled,
            pids_max: cfg.pids_max,
            memory_max: cfg.memory_max,
            cpu_max: cfg.cpu_max,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerRuntimeConfig {
    pub kill_timeout_seconds: Option<u64>,
}
