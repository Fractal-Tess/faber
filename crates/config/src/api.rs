use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub cors: CorsConfig,
    pub request: RequestConfig,
    pub auth: AuthConfig,
    pub endpoints: EndpointsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub enable_cors: bool,
    pub cors_allowed_origins: String,
    pub cors_allowed_methods: String,
    pub cors_allowed_headers: String,
    pub cors_allow_credentials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConfig {
    pub max_request_size_kb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enable: String,     // Can be "env:FABER_AUTH_ENABLE|false" format
    pub secret_key: String, // Can be "env:FABER_AUTH_SECRET_KEY" format
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointsConfig {
    pub health_endpoint: String,
    pub execute_endpoint: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
            cors: CorsConfig::default(),
            request: RequestConfig::default(),
            auth: AuthConfig::default(),
            endpoints: EndpointsConfig::default(),
        }
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enable_cors: false,
            cors_allowed_origins: "*".to_string(),
            cors_allowed_methods: "GET,POST,OPTIONS".to_string(),
            cors_allowed_headers: "*".to_string(),
            cors_allow_credentials: false,
        }
    }
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            max_request_size_kb: 10240, // 10MB
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enable: "env:FABER_AUTH_ENABLE|false".to_string(),
            secret_key: "env:FABER_AUTH_SECRET_KEY".to_string(),
        }
    }
}

impl Default for EndpointsConfig {
    fn default() -> Self {
        Self {
            health_endpoint: "/health".to_string(),
            execute_endpoint: "/execute-tasks".to_string(),
        }
    }
}
