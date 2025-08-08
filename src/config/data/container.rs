use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fs::File;

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

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemConfig {
    pub mounts: MountsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MountsConfig {
    #[serde(deserialize_with = "deserialize_folder_mounts")]
    pub folders: Vec<FolderMount>,
    #[serde(deserialize_with = "deserialize_tmpfs_mounts")]
    pub tmpfs: Vec<TempfsMount>,
    #[serde(deserialize_with = "deserialize_device_mounts")]
    pub devices: Vec<DeviceMount>,
    #[serde(deserialize_with = "deserialize_file_mounts")]
    pub files: Vec<FileMount>,
}

#[derive(Debug, Clone)]
pub struct FolderMount {
    pub name: String,
    pub source: String,
    pub target: String,
    pub permissions: FolderPermissions,
}

#[derive(Debug, Clone)]
pub struct TempfsMount {
    pub name: String,
    pub target: String,
    pub options: String,
}

#[derive(Debug, Clone)]
pub struct DeviceMount {
    pub name: String,
    pub source: String,
    pub target: String,
    pub permissions: DevicePermissions,
}

#[derive(Debug, Clone)]
pub struct FileMount {
    pub name: String,
    pub source: String,
    pub target: String,
    pub permissions: FilePermissions,
}

#[derive(Debug, Clone)]
pub enum DevicePermissions {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone)]
pub enum FilePermissions {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FolderPermissions {
    ReadOnly,
    ReadWrite,
}

// Custom deserialization functions
fn deserialize_folder_mounts<'de, D>(deserializer: D) -> Result<Vec<FolderMount>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    let mut mounts = Vec::new();

    for (name, values) in map {
        if values.len() >= 2 {
            let permissions = if values.len() >= 3 && values[2] == "ro" {
                FolderPermissions::ReadOnly
            } else {
                FolderPermissions::ReadWrite
            };

            let mount = FolderMount {
                name: name.clone(),
                source: values[0].clone(),
                target: values[1].clone().trim_start_matches('/').to_string(),
                permissions,
            };
            mounts.push(mount);
        } else {
        }
    }

    Ok(mounts)
}

fn deserialize_tmpfs_mounts<'de, D>(deserializer: D) -> Result<Vec<TempfsMount>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    let mut mounts = Vec::new();

    for (name, values) in map {
        if values.len() == 2 {
            let mount = TempfsMount {
                name: name.clone(),
                target: values[0].clone().trim_start_matches('/').to_string(),
                options: values[1].clone(),
            };
            mounts.push(mount);
        } else {
        }
    }

    Ok(mounts)
}

fn deserialize_device_mounts<'de, D>(deserializer: D) -> Result<Vec<DeviceMount>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    let mut mounts = Vec::new();

    for (name, values) in map {
        if values.len() >= 2 {
            let permissions = if values.len() >= 3 && values[2] == "rw" {
                DevicePermissions::ReadWrite
            } else {
                DevicePermissions::ReadOnly
            };

            let mount = DeviceMount {
                name: name.clone(),
                source: values[0].clone(),
                target: values[1].clone().trim_start_matches('/').to_string(),
                permissions,
            };
            mounts.push(mount);
        } else {
        }
    }

    Ok(mounts)
}

fn deserialize_file_mounts<'de, D>(deserializer: D) -> Result<Vec<FileMount>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    let mut mounts = Vec::new();

    for (name, values) in map {
        if values.len() >= 2 {
            let permissions = if values.len() >= 3 && values[2] == "rw" {
                FilePermissions::ReadWrite
            } else {
                FilePermissions::ReadOnly
            };

            let mount = FileMount {
                name: name.clone(),
                source: values[0].clone(),
                target: values[1].clone().trim_start_matches('/').to_string(),
                permissions,
            };
            mounts.push(mount);
        } else {
        }
    }

    Ok(mounts)
}
