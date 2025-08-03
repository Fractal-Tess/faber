use utoipa::OpenApi;

/// Comprehensive API Documentation for Faber Code Execution Platform
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Faber - Code Execution API",
        description = "A secure sandbox API for executing code in isolated environments. Submit source files and commands to compile, run, and interact with code in any supported programming language. Features include secure sandbox execution, multi-language support, file management, sequential task execution, resource monitoring, and configurable authentication.",
        version = "2.0.0",
        contact(
            name = "Faber Development Team",
            email = "dev@faber.dev",
            url = "https://github.com/faber-dev/faber"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    paths(
        crate::handlers::health_check,
        crate::handlers::run_code
    ),
    components(
        schemas(
            // Response types
            crate::handlers::HealthResponse,
            // Executor types
            crate::executor::ExecutionRequest,
            crate::executor::ExecutionResult,
            crate::executor::Task,
            crate::executor::TaskResult,
            crate::executor::FileSource
        )
    ),
    tags(
        (
            name = "health",
            description = "Health Check Endpoints - Monitor API status and availability"
        ),

        (
            name = "execution",
            description = "Code Execution Endpoints - Secure sandbox code execution with resource monitoring"
        )
    )
)]
pub struct ApiDoc;
