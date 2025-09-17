use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CgroupConfig {
    pub pids_max: Option<u64>,
    pub memory_max: Option<String>,
    pub cpu_max: Option<String>,
}

pub type TaskGroup = Vec<ExecutionStep>;

#[derive(Debug, Clone)]
pub enum ExecutionStep {
    Single(Task),
    Parallel(Vec<Task>),
}

impl serde::Serialize for ExecutionStep {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ExecutionStep::Single(task) => task.serialize(serializer),
            ExecutionStep::Parallel(tasks) => tasks.serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ExecutionStep {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::Object(_) => {
                let task = Task::deserialize(value).map_err(Error::custom)?;
                Ok(ExecutionStep::Single(task))
            }
            serde_json::Value::Array(_) => {
                let tasks = Vec::<Task>::deserialize(value).map_err(Error::custom)?;
                Ok(ExecutionStep::Parallel(tasks))
            }
            _ => Err(Error::custom(
                "ExecutionStep must be either an object (Single) or an array (Parallel)",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub stdin: Option<String>,
    pub files: Option<HashMap<String, String>>,
    pub working_dir: Option<String>,
}
