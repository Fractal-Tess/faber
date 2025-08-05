use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub cors: CorsConfig,
    pub request: RequestConfig,
    pub auth: AuthConfig,
    pub endpoints: EndpointsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CorsConfig {
    pub enable_cors: bool,
    pub cors_allowed_origins: String,
    pub cors_allowed_methods: String,
    pub cors_allowed_headers: String,
    pub cors_allow_credentials: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestConfig {
    pub max_request_size_kb: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub enable: bool,
    pub secret_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EndpointsConfig {
    pub health_endpoint: String,
    pub execute_endpoint: String,
}
