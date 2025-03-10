use axum::{http::StatusCode, response::IntoResponse};
use serde_json::json;

use super::utils::response_handler;

pub async fn health() -> impl IntoResponse {
    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({
            "health": "ok",
            "server_time": chrono::Utc::now().to_rfc3339(),
        })),
        None,
    )
}
