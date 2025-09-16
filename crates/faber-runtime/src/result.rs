use serde::de::Error;
use serde::{Deserialize, Serialize};

pub type TaskGroupResult = Vec<ExecutionStepResult>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum RuntimeResult {
    Success(TaskGroupResult),
    ContainerSetupFailed { error: String },
}

#[derive(Debug, Clone)]
pub enum ExecutionStepResult {
    Single(TaskResult),
    Parallel(Vec<TaskResult>),
}

impl serde::Serialize for ExecutionStepResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ExecutionStepResult::Single(task_result) => task_result.serialize(serializer),
            ExecutionStepResult::Parallel(task_results) => task_results.serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ExecutionStepResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::Object(_) => {
                let task_result = TaskResult::deserialize(value).map_err(Error::custom)?;
                Ok(ExecutionStepResult::Single(task_result))
            }
            serde_json::Value::Array(_) => {
                let task_results = Vec::<TaskResult>::deserialize(value).map_err(Error::custom)?;
                Ok(ExecutionStepResult::Parallel(task_results))
            }
            _ => Err(Error::custom(
                "ExecutionStepResult must be either an object (Single) or an array (Parallel)",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TaskResult {
    Completed {
        stdout: String,
        stderr: String,
        exit_code: i32,
        stats: TaskResultStats,
    },
    Failed {
        error: String,
        stats: TaskResultStats,
    },
}

impl serde::Serialize for TaskResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        match self {
            TaskResult::Completed {
                stdout,
                stderr,
                exit_code,
                stats,
            } => {
                let mut state = serializer.serialize_struct("TaskResult", 4)?;
                state.serialize_field("stdout", stdout)?;
                state.serialize_field("stderr", stderr)?;
                state.serialize_field("exit_code", exit_code)?;
                state.serialize_field("stats", stats)?;
                state.end()
            }
            TaskResult::Failed { error, stats } => {
                let mut state = serializer.serialize_struct("TaskResult", 2)?;
                state.serialize_field("error", error)?;
                state.serialize_field("stats", stats)?;
                state.end()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for TaskResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, MapAccess, Visitor};
        use std::fmt;

        struct TaskResultVisitor;

        impl<'de> Visitor<'de> for TaskResultVisitor {
            type Value = TaskResult;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a TaskResult object")
            }

            fn visit_map<V>(self, mut map: V) -> Result<TaskResult, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut stdout = None;
                let mut stderr = None;
                let mut exit_code = None;
                let mut error = None;
                let mut stats = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "stdout" => stdout = Some(map.next_value()?),
                        "stderr" => stderr = Some(map.next_value()?),
                        "exit_code" => exit_code = Some(map.next_value()?),
                        "error" => error = Some(map.next_value()?),
                        "stats" => stats = Some(map.next_value()?),
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                let stats = stats.ok_or_else(|| Error::missing_field("stats"))?;

                if let Some(error) = error {
                    Ok(TaskResult::Failed { error, stats })
                } else {
                    let stdout = stdout.ok_or_else(|| Error::missing_field("stdout"))?;
                    let stderr = stderr.ok_or_else(|| Error::missing_field("stderr"))?;
                    let exit_code = exit_code.ok_or_else(|| Error::missing_field("exit_code"))?;
                    Ok(TaskResult::Completed {
                        stdout,
                        stderr,
                        exit_code,
                        stats,
                    })
                }
            }
        }

        deserializer.deserialize_struct(
            "TaskResult",
            &["stdout", "stderr", "exit_code", "error", "stats"],
            TaskResultVisitor,
        )
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TaskResultStats {
    pub memory_peak_bytes: u64,
    pub cpu_usage_percent: f64,
    pub execution_time_ms: u64,
}
