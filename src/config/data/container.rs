use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerConfig {
    pub filesystem: ContainerFilesystemConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerFilesystemConfig {
    pub base_dir: String,
}
