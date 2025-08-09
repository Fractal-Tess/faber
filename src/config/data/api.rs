use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub max_concurrency: usize,
    pub auth: AuthConfig,
    pub endpoints: EndpointsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub enable: bool,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EndpointsConfig {
    pub health_endpoint: String,
    pub task_execution_endpoint: String,
}
