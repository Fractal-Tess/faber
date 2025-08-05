use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemConfig {
    pub mounts: MountsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MountsConfig {
    #[serde(deserialize_with = "deserialize_readable_mounts")]
    pub readable: Vec<ReadOnlyMount>,
    #[serde(deserialize_with = "deserialize_writable_mounts")]
    pub writable: Vec<ReadWriteMount>,
    #[serde(deserialize_with = "deserialize_tmpfs_mounts")]
    pub tmpfs: Vec<TempfsMount>,
}

#[derive(Debug, Clone)]
pub struct ReadOnlyMount {
    pub name: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct ReadWriteMount {
    pub name: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct TempfsMount {
    pub name: String,
    pub target: String,
    pub options: String,
}

// Custom deserialization functions
fn deserialize_readable_mounts<'de, D>(deserializer: D) -> Result<Vec<ReadOnlyMount>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    let mut mounts = Vec::new();

    for (name, paths) in map {
        if paths.len() == 2 {
            mounts.push(ReadOnlyMount {
                name,
                source: paths[0].clone(),
                target: paths[1].clone(),
            });
        }
    }

    Ok(mounts)
}

fn deserialize_writable_mounts<'de, D>(deserializer: D) -> Result<Vec<ReadWriteMount>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
    let mut mounts = Vec::new();

    for (name, paths) in map {
        if paths.len() == 2 {
            mounts.push(ReadWriteMount {
                name,
                source: paths[0].clone(),
                target: paths[1].clone(),
            });
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
            mounts.push(TempfsMount {
                name,
                target: values[0].clone(),
                options: values[1].clone(),
            });
        }
    }

    Ok(mounts)
}

impl ReadOnlyMount {
    pub fn new(name: String, source: String, target: String) -> Self {
        Self {
            name,
            source,
            target,
        }
    }
}

impl ReadWriteMount {
    pub fn new(name: String, source: String, target: String) -> Self {
        Self {
            name,
            source,
            target,
        }
    }
}

impl TempfsMount {
    pub fn new(name: String, target: String, options: String) -> Self {
        Self {
            name,
            target,
            options,
        }
    }
}
