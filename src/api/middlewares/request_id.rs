use axum::{extract::Request, middleware::Next, response::Response};

pub type RequestId = String;

pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id: RequestId = uuid::Uuid::new_v4().to_string();
    request.extensions_mut().insert(request_id);
    next.run(request).await
}
