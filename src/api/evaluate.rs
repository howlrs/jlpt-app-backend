use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use log::info;
use serde::Deserialize;
use serde_json::json;

use crate::{api::utils::response_handler, models::evaluate::Vote};

#[derive(Deserialize)]
pub struct PathParams {
    vote: String,
}

#[derive(Deserialize)]
pub struct QueryParams {
    parent_id: Option<String>,
    child_id: Option<String>,
}

/// # vote
///
/// ## 概要
/// 問題に対する評価
///
/// ## HTTP情報
/// - **メソッド**: GET
/// - **パス**: /api/evaluate/{vote}
/// - **認証**: 不要
///
/// ## パスパラメータ
/// - `vote`: 評価 (String) good/bad - 評価を指定する
///
/// ## クエリ
/// - `parent_id`: 対象 (String) - 問題のIDなど
/// - `child_id`: 対象 (String) - 問題の子IDなど
///
/// ## レスポンス
/// ### 成功時
/// - **ステータスコード**: 200 OK
/// - **形式**: JSON
/// - **内容**:
///   ```json
///   {
///     "status": "success",
///     "message": "success",
///     "data": {
///      "vote": "good",
///        "parent_id": "1",
///       "child_id": "1"
///    }
///   }
///   ```
///
/// ### エラー時
/// - **ステータスコード**: 404 Not Found
/// - **内容**: リソースが存在しない場合のエラーメッセージ
///
/// ## 例
/// POST /api/evalute/good?parent_id=1&child_id=1
///
/// ## 関連エンドポイント
/// - `get`: 評価取得エンドポイント
pub async fn vote(
    Path(path_params): Path<PathParams>,
    Query(query_params): Query<QueryParams>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    let parent_id = query_params.parent_id.as_deref().unwrap_or_default();
    let child_id = query_params.child_id.as_deref().unwrap_or_default();

    info!(
        "vote: {}, parent_id: {}, child_id: {}",
        path_params.vote, parent_id, child_id
    );

    let vote = Vote::new(
        path_params.vote.clone(),
        Some("questions".to_string()),
        parent_id.to_string(),
        child_id.to_string(),
    );

    match db
        .client
        .fluent()
        .insert()
        .into("votes")
        .document_id(vote.id())
        .object(&vote)
        .execute::<Vote>()
        .await
    {
        Ok(_) => response_handler(
            StatusCode::OK,
            "success".to_string(),
            Some(json!({
                "vote": &path_params.vote,
                "parent_id": parent_id,
                "child_id": child_id,
            })),
            None,
        ),
        Err(e) => response_handler(
            StatusCode::INTERNAL_SERVER_ERROR,
            "error".to_string(),
            None,
            Some(e.to_string()),
        ),
    }
}
