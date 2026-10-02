use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::utils::generate_random_string;

/// What a sandbox sees of the surrounding image by default: the toolchains,
/// and the few files under /etc that make them usable (the alternatives
/// links behind commands such as `cc` and `java`, the loader cache, and the
/// user database that names the task's UID). Nothing else of /etc: no
/// resolver configuration, hosts file or service configuration.
pub const DEFAULT_READONLY_PATHS: [&str; 8] = [
    "/bin",
    "/lib",
    "/lib64",
    "/usr",
    "/etc/alternatives",
    "/etc/ld.so.cache",
    "/etc/passwd",
    "/etc/group",
];

/// Directory holding one root directory per running request.
pub(crate) const SANDBOX_ROOTS: &str = "/tmp/faber";

#[derive(Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub(crate) container_root_dir: PathBuf,
    pub(crate) workdir: PathBuf,
    pub(crate) tmpdir_size: String,
    pub(crate) workdir_size: String,
    pub(crate) bind_mounts_ro: Vec<String>,
    pub(crate) hostname: String,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        let id = generate_random_string(12);
        let container_root_dir = PathBuf::from(SANDBOX_ROOTS).join(id);
        let bind_mounts_ro = DEFAULT_READONLY_PATHS.map(String::from).to_vec();
        let workdir = PathBuf::from("/faber");
        let tmpdir_size = "128M".to_string();
        let workdir_size = "128M".to_string();
        let hostname = "faber".to_string();

        Self {
            container_root_dir,
            workdir,
            tmpdir_size,
            workdir_size,
            bind_mounts_ro,
            hostname,
        }
    }
}
