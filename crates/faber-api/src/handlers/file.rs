use axum::{
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Json},
};
use bytes::Bytes;
use faber_store::{FileInfo, FileMetadata};
use serde::Serialize;
use tracing::{debug, error, warn};

use crate::state::AppState;

const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub file_id: String,
    pub size: u64,
    pub already_exists: bool,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut content: Option<Bytes> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to read multipart field: {}", e);
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Failed to read multipart data: {}", e),
            }),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                content_type = field.content_type().map(|s| s.to_string());

                let mut buffer = Vec::new();
                let mut chunk_stream = field;
                while let Some(chunk) = chunk_stream.chunk().await.map_err(|e| {
                    error!("Failed to read file chunk: {}", e);
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: format!("Failed to read file content: {}", e),
                        }),
                    )
                })? {
                    buffer.extend_from_slice(&chunk);
                    if buffer.len() > MAX_FILE_SIZE {
                        warn!("File upload rejected: size exceeds {} bytes", MAX_FILE_SIZE);
                        return Err((
                            StatusCode::PAYLOAD_TOO_LARGE,
                            Json(ErrorResponse {
                                error: format!(
                                    "File too large. Maximum size is {} bytes",
                                    MAX_FILE_SIZE
                                ),
                            }),
                        ));
                    }
                }
                content = Some(Bytes::from(buffer));
            }
            _ => {
                debug!("Ignoring unknown field: {}", name);
            }
        }
    }

    let content = content.ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "No file provided".to_string(),
        }),
    ))?;

    let mut metadata = FileMetadata::new(content.len() as u64);
    if let Some(name) = filename {
        metadata = metadata.with_filename(name);
    }
    if let Some(ct) = content_type {
        metadata = metadata.with_content_type(ct);
    }

    let result = state.file_store.put(content, metadata).await.map_err(|e| {
        error!("Failed to store file: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to store file: {}", e),
            }),
        )
    })?;

    Ok(Json(UploadResponse {
        file_id: result.file_id.to_string(),
        size: result.size,
        already_exists: result.already_exists,
    }))
}

pub async fn list_files(
    State(state): State<AppState>,
) -> Result<Json<ListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let files = state.file_store.list().await.map_err(|e| {
        error!("Failed to list files: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to list files: {}", e),
            }),
        )
    })?;

    Ok(Json(ListResponse { files }))
}

pub async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let file_id = faber_store::FileId::from(id);

    let file = state.file_store.get(&file_id).await.map_err(|e| match e {
        faber_store::StoreError::NotFound(_) => {
            warn!("File not found: {}", file_id);
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("File not found: {}", file_id),
                }),
            )
        }
        _ => {
            error!("Failed to retrieve file: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to retrieve file: {}", e),
                }),
            )
        }
    })?;

    let mut headers = HeaderMap::new();
    let content_type_value = file
        .metadata
        .content_type
        .unwrap_or_else(|| "application/octet-stream".to_string())
        .parse()
        .map_err(|e| {
            error!("Invalid content type header: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Invalid content type".to_string(),
                }),
            )
        })?;
    headers.insert(header::CONTENT_TYPE, content_type_value);

    let content_disposition = file.metadata.filename.as_ref().map_or_else(
        || "attachment".to_string(),
        |n| {
            let escaped: String = n
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
                .collect();
            if escaped.is_empty() {
                "attachment".to_string()
            } else {
                format!("attachment; filename=\"{}\"", escaped)
            }
        },
    );
    let content_disposition_value = content_disposition.parse().map_err(|e| {
        error!("Invalid content disposition header: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Invalid content disposition".to_string(),
            }),
        )
    })?;
    headers.insert(header::CONTENT_DISPOSITION, content_disposition_value);

    Ok((headers, file.content))
}

pub async fn delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let file_id = faber_store::FileId::from(id);

    let deleted = state.file_store.delete(&file_id).await.map_err(|e| {
        error!("Failed to delete file: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to delete file: {}", e),
            }),
        )
    })?;

    if deleted {
        debug!("Deleted file: {}", file_id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        warn!("File not found for deletion: {}", file_id);
        Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("File not found: {}", file_id),
            }),
        ))
    }
}
