use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemConfig {
    pub mounts: MountsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MountsConfig {
    pub readable: std::collections::HashMap<String, Vec<String>>,
    pub tmpfs: std::collections::HashMap<String, Vec<String>>,
}
