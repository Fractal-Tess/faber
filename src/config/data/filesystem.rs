use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use tracing::{debug, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemConfig {
    pub mounts: MountsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MountsConfig {
    #[serde(deserialize_with = "deserialize_folder_mounts")]
    pub folders: Vec<ReadOnlyMount>,
    #[serde(deserialize_with = "deserialize_tmpfs_mounts")]
    pub tmpfs: Vec<TempfsMount>,
    #[serde(deserialize_with = "deserialize_device_mounts")]
    pub devices: Vec<DeviceMount>,
    #[serde(default, deserialize_with = "deserialize_file_mounts")]
    pub files: Vec<FileMount>,
}

#[derive(Debug, Clone)]
pub struct ReadOnlyMount {
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

#[derive(Debug, Clone)]
pub enum FolderPermissions {
    ReadOnly,
    ReadWrite,
}

// Custom deserialization functions
fn deserialize_folder_mounts<'de, D>(deserializer: D) -> Result<Vec<ReadOnlyMount>, D::Error>
where
    D: Deserializer<'de>,
{
    debug!("=== Deserializing folder mounts ===");
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    debug!("Raw folder mounts map: {:?}", map);
    let mut mounts = Vec::new();

    for (name, values) in map {
        debug!(
            "Processing folder mount '{}' with values: {:?}",
            name, values
        );
        if values.len() >= 2 {
            let permissions = if values.len() >= 3 && values[2] == "ro" {
                FolderPermissions::ReadOnly
            } else {
                FolderPermissions::ReadWrite
            };

            let mount = ReadOnlyMount {
                name: name.clone(),
                source: values[0].clone(),
                target: values[1].clone().trim_start_matches('/').to_string(),
                permissions,
            };
            debug!("Created folder mount: {:?}", mount);
            mounts.push(mount);
        } else {
            warn!(
                "Folder mount '{}' has {} values, expected at least 2, skipping",
                name,
                values.len()
            );
        }
    }
    debug!("Final folder mounts: {:?}", mounts);

    Ok(mounts)
}

fn deserialize_tmpfs_mounts<'de, D>(deserializer: D) -> Result<Vec<TempfsMount>, D::Error>
where
    D: Deserializer<'de>,
{
    debug!("=== Deserializing tmpfs mounts ===");
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    debug!("Raw tmpfs mounts map: {:?}", map);
    let mut mounts = Vec::new();

    for (name, values) in map {
        debug!(
            "Processing tmpfs mount '{}' with values: {:?}",
            name, values
        );
        if values.len() == 2 {
            let mount = TempfsMount {
                name: name.clone(),
                target: values[0].clone().trim_start_matches('/').to_string(),
                options: values[1].clone(),
            };
            debug!("Created tmpfs mount: {:?}", mount);
            mounts.push(mount);
        } else {
            warn!(
                "Tmpfs mount '{}' has {} values, expected 2, skipping",
                name,
                values.len()
            );
        }
    }
    debug!("Final tmpfs mounts: {:?}", mounts);

    Ok(mounts)
}

fn deserialize_device_mounts<'de, D>(deserializer: D) -> Result<Vec<DeviceMount>, D::Error>
where
    D: Deserializer<'de>,
{
    debug!("=== Deserializing device mounts ===");
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    debug!("Raw device mounts map: {:?}", map);
    let mut mounts = Vec::new();

    for (name, values) in map {
        debug!(
            "Processing device mount '{}' with values: {:?}",
            name, values
        );
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
            debug!("Created device mount: {:?}", mount);
            mounts.push(mount);
        } else {
            warn!(
                "Device mount '{}' has {} values, expected at least 2, skipping",
                name,
                values.len()
            );
        }
    }
    debug!("Final device mounts: {:?}", mounts);

    Ok(mounts)
}

fn deserialize_file_mounts<'de, D>(deserializer: D) -> Result<Vec<FileMount>, D::Error>
where
    D: Deserializer<'de>,
{
    debug!("=== Deserializing file mounts ===");
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    debug!("Raw file mounts map: {:?}", map);
    let mut mounts = Vec::new();

    for (name, values) in map {
        debug!("Processing file mount '{}' with values: {:?}", name, values);
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
            debug!("Created file mount: {:?}", mount);
            mounts.push(mount);
        } else {
            warn!(
                "File mount '{}' has {} values, expected at least 2, skipping",
                name,
                values.len()
            );
        }
    }
    debug!("Final file mounts: {:?}", mounts);

    Ok(mounts)
}
