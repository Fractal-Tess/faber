use std::path::{Path, PathBuf};

use crate::runtime::Runtime;
use crate::types::{CgroupConfig, Mount};

pub struct RuntimeBuilder {
    pub(crate) container_root: PathBuf,
    pub(crate) mounts: Vec<Mount>,
    pub(crate) cgroup: Option<CgroupConfig>,
    pub(crate) work_dir: String,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mounts(mut self, mounts: Vec<Mount>) -> Self {
        self.mounts = mounts;
        self
    }

    pub fn with_cgroups(mut self, cfg: CgroupConfig) -> Self {
        self.cgroup = Some(cfg);
        self
    }

    pub fn with_workdir(mut self, work_dir: String) -> Self {
        self.work_dir = work_dir;
        self
    }

    pub fn with_container_root(mut self, container_root: impl Into<PathBuf>) -> Self {
        self.container_root = container_root.into();
        self
    }

    pub fn build(self) -> Runtime {
        Runtime::from_builder_parts(self.container_root, self.mounts, self.cgroup, self.work_dir)
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self {
            container_root: "/tmp/faber/containers".into(),
            mounts: default_mounts(),
            cgroup: None,
            work_dir: "/faber".into(),
        }
    }
}

fn default_mounts() -> Vec<Mount> {
    use nix::mount::MsFlags;
    let ro = vec![MsFlags::MS_BIND, MsFlags::MS_REC, MsFlags::MS_RDONLY];
    let mut mounts: Vec<Mount> = vec![
        Mount {
            source: "/bin".into(),
            target: "/bin".into(),
            flags: ro.clone(),
            options: vec![],
            data: None,
        },
        Mount {
            source: "/lib".into(),
            target: "/lib".into(),
            flags: ro.clone(),
            options: vec![],
            data: None,
        },
        Mount {
            source: "/usr/bin".into(),
            target: "/usr/bin".into(),
            flags: ro.clone(),
            options: vec![],
            data: None,
        },
    ];
    if Path::new("/lib64").exists() {
        mounts.push(Mount {
            source: "/lib64".into(),
            target: "/lib64".into(),
            flags: ro,
            options: vec![],
            data: None,
        });
    }
    mounts
}
