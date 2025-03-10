use axum::{http::StatusCode, response::IntoResponse};
use serde_json::json;

use super::utils::response_handler;

/// # health
///
/// APIエンドポイントの説明: このエンドポイントはサーバーのヘルスステータスを返します。
///
/// ## パラメータ
///
/// なし - このエンドポイントはパラメータを必要としません。
///
/// ## 返り値
///
/// 以下の形式のJSONレスポンスを返します：
///
/// ```json
/// {
///   "success": true,
///   "message": "success",
///   "data": {
///     "health": "ok",
///     "server_time": "2023-12-31T23:59:59.999Z"
///   }
/// }
/// ```
///
/// - `health`: サーバーの状態を表す文字列
/// - `server_time`: サーバーの現在時刻（RFC 3339形式）
///
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
