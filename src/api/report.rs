use std::sync::Arc;

use axum::{extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde_json::json;
use log::error;

use crate::api::utils::response_handler;
use crate::common::database::Database;
use crate::models::claim::{AdminClaims, Claims};
use crate::models::report::QuestionReport;

/// POST /api/questions/:id/report
/// ログイン済ユーザが問題を「報告」する。同じ質問への二重報告は 409 Conflict。
pub async fn report_question(
    State(db): State<Arc<Database>>,
    claims: Claims,
    Path(question_id): Path<String>,
) -> impl IntoResponse {
    let doc_id = QuestionReport::doc_id(&question_id, &claims.user_id);

    // 重複チェック
    if let Ok(Some(_)) = db.read::<QuestionReport>("reports", &doc_id).await {
        return response_handler(
            StatusCode::CONFLICT,
            "error".to_string(),
            None,
            Some("既に報告済みです".to_string()),
        );
    }

    let report = QuestionReport::new(question_id, claims.user_id);
    match db.create::<QuestionReport>("reports", &doc_id, report).await {
        Ok(_) => response_handler(
            StatusCode::OK,
            "success".to_string(),
            Some(json!({"reported": true})),
            None,
        ),
        Err(e) => {
            error!("報告保存失敗: {:?}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some("報告の保存に失敗しました".to_string()),
            )
        }
    }
}

/// GET /api/admin/reports
/// Admin専用。question_id ごとの報告件数を降順で返す。
pub async fn list_reports(
    State(db): State<Arc<Database>>,
    _claims: AdminClaims,
) -> impl IntoResponse {
    // 全 reports を取得して question_id 別に集計
    match db.read_all::<QuestionReport>("reports", Some(1000)).await {
        Ok(reports) => {
            let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for r in &reports {
                *counts.entry(r.question_id.clone()).or_insert(0) += 1;
            }
            let mut sorted: Vec<_> = counts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));

            let items: Vec<serde_json::Value> = sorted.iter()
                .map(|(id, count)| json!({"question_id": id, "report_count": count}))
                .collect();
            response_handler(
                StatusCode::OK,
                "success".to_string(),
                Some(json!(items)),
                None,
            )
        }
        Err(e) => {
            error!("報告一覧取得失敗: {:?}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some("報告一覧の取得に失敗しました".to_string()),
            )
        }
    }
}
