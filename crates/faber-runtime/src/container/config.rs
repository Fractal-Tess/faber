use std::path::PathBuf;

use crate::utils::generate_random_string;

pub struct ContainerConfig {
    pub(crate) id: String,
    pub(crate) container_root_dir: PathBuf,
    pub(crate) workdir: PathBuf,
    pub(crate) tmpdir_size: String,
    pub(crate) workdir_size: String,
    pub(crate) bind_mounts_ro: Vec<&'static str>,
    pub(crate) bind_mounts_rw: Vec<&'static str>,
    pub(crate) hostname: String,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        let id = generate_random_string(12);
        let container_root_dir = PathBuf::from(format!("/tmp/faber/{}", id));
        let bind_mounts_ro = vec!["/bin", "/lib", "/lib64", "/usr"];
        let bind_mounts_rw = vec![""];
        let workdir = PathBuf::from("/faber");
        let tmpdir_size = "128M".to_string();
        let workdir_size = "128M".to_string();
        let hostname = "faber".to_string();

        Self {
            id,
            container_root_dir,
            workdir,
            tmpdir_size,
            workdir_size,
            bind_mounts_ro,
            bind_mounts_rw,
            hostname,
        }
    }
}
