use utoipa::OpenApi;

/// API Documentation for Faber
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Faber API",
        description = "A simple API with authentication middleware",
        version = "1.0.0",
        contact(
            name = "Faber Team",
            email = "team@faber.example.com"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    paths(
        crate::api::handlers::health_check,
        crate::api::handlers::protected
    ),
    components(
        schemas(
            crate::api::handlers::HealthResponse,
            crate::api::handlers::ProtectedResponse
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "protected", description = "Protected endpoints requiring authentication")
    )
)]
pub struct ApiDoc;
