use axum::{http::StatusCode, response::IntoResponse};

use super::utils::response_handler;

pub async fn read() -> impl IntoResponse {
    response_handler(StatusCode::OK, "success".to_string(), None, None)
}
