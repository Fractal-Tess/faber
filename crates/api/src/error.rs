use serde::Serialize;

#[derive(Serialize)]
pub struct ApiExecutionResponseError {
    pub error: String,
}
