use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemConfig {
    pub mounts: MountsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountsConfig {
    pub readable: std::collections::HashMap<String, Vec<String>>,
    pub tmpfs: std::collections::HashMap<String, Vec<String>>,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            mounts: MountsConfig::default(),
        }
    }
}

impl Default for MountsConfig {
    fn default() -> Self {
        let mut readable = std::collections::HashMap::new();
        readable.insert(
            "bin".to_string(),
            vec!["/bin".to_string(), "/bin".to_string()],
        );
        readable.insert(
            "lib".to_string(),
            vec!["/lib".to_string(), "/lib".to_string()],
        );
        readable.insert(
            "lib64".to_string(),
            vec!["/lib64".to_string(), "/lib64".to_string()],
        );
        readable.insert(
            "usr".to_string(),
            vec!["/usr".to_string(), "/usr".to_string()],
        );
        readable.insert(
            "dev_null".to_string(),
            vec!["/dev/null".to_string(), "/dev/null".to_string()],
        );
        readable.insert(
            "dev_random".to_string(),
            vec!["/dev/random".to_string(), "/dev/random".to_string()],
        );
        readable.insert(
            "dev_urandom".to_string(),
            vec!["/dev/urandom".to_string(), "/dev/urandom".to_string()],
        );
        readable.insert(
            "dev_zero".to_string(),
            vec!["/dev/zero".to_string(), "/dev/zero".to_string()],
        );
        readable.insert(
            "dev_full".to_string(),
            vec!["/dev/full".to_string(), "/dev/full".to_string()],
        );

        let mut tmpfs = std::collections::HashMap::new();
        tmpfs.insert(
            "work_tmpfs".to_string(),
            vec!["/work".to_string(), "size=256m,nr_inodes=4k".to_string()],
        );
        tmpfs.insert(
            "tmp_tmpfs".to_string(),
            vec!["/tmp".to_string(), "size=128m,nr_inodes=4k".to_string()],
        );

        Self { readable, tmpfs }
    }
}
