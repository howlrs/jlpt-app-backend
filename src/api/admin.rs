use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use log::error;
use serde::Deserialize;
use serde_json::json;
use tokio_stream::StreamExt;

use crate::{
    api::utils::response_handler,
    models::{
        claim::AdminClaims,
        evaluate::Vote,
        question::Question,
    },
};

#[derive(Deserialize)]
pub struct QuestionPath {
    id: String,
}

/// GET /api/admin/votes/summary
/// 投票の集計サマリーを返す
pub async fn votes_summary(
    _admin: AdminClaims,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    // 全投票を取得
    let votes = match db
        .client
        .fluent()
        .list()
        .from("votes")
        .obj::<Vote>()
        .stream_all()
        .await
    {
        Ok(stream) => stream.collect::<Vec<Vote>>().await,
        Err(e) => {
            error!("Failed to fetch votes: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e.to_string()),
            );
        }
    };

    let total_votes = votes.len();

    // parent_id ごとに good/bad を集計
    let mut aggregation: HashMap<String, (i64, i64)> = HashMap::new();
    for v in &votes {
        let entry = aggregation.entry(v.parent_id.clone()).or_insert((0, 0));
        if v.vote == "good" {
            entry.0 += 1;
        } else if v.vote == "bad" {
            entry.1 += 1;
        }
    }

    let total_questions_voted = aggregation.len();
    let good_count: i64 = aggregation.values().map(|(g, _)| g).sum();
    let bad_count: i64 = aggregation.values().map(|(_, b)| b).sum();
    let bad_questions_count = aggregation.values().filter(|(g, b)| b >= g).count();

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({
            "total_votes": total_votes,
            "total_questions_voted": total_questions_voted,
            "good_count": good_count,
            "bad_count": bad_count,
            "bad_questions_count": bad_questions_count,
        })),
        None,
    )
}

/// GET /api/admin/questions/bad
/// bad >= good の問題一覧を返す
pub async fn bad_questions(
    _admin: AdminClaims,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    // 全投票を取得
    let votes = match db
        .client
        .fluent()
        .list()
        .from("votes")
        .obj::<Vote>()
        .stream_all()
        .await
    {
        Ok(stream) => stream.collect::<Vec<Vote>>().await,
        Err(e) => {
            error!("Failed to fetch votes: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e.to_string()),
            );
        }
    };

    // parent_id ごとに good/bad を集計
    let mut aggregation: HashMap<String, (i64, i64)> = HashMap::new();
    for v in &votes {
        let entry = aggregation.entry(v.parent_id.clone()).or_insert((0, 0));
        if v.vote == "good" {
            entry.0 += 1;
        } else if v.vote == "bad" {
            entry.1 += 1;
        }
    }

    // bad >= good の問題を抽出
    let mut bad_entries: Vec<(String, i64, i64)> = aggregation
        .into_iter()
        .filter(|(_, (g, b))| b >= g)
        .map(|(id, (g, b))| (id, g, b))
        .collect();

    // bad_count 降順でソート
    bad_entries.sort_by(|a, b| b.2.cmp(&a.2));

    // 各問題の詳細を取得
    let mut results = Vec::new();
    for (parent_id, good, bad) in &bad_entries {
        let question = match db
            .read::<Question>("questions", parent_id)
            .await
        {
            Ok(Some(q)) => json!(q),
            Ok(None) => json!({ "id": parent_id, "error": "question not found" }),
            Err(e) => {
                error!("Failed to fetch question {}: {}", parent_id, e);
                json!({ "id": parent_id, "error": e })
            }
        };

        let total = good + bad;
        let bad_rate = if total > 0 {
            *bad as f64 / total as f64
        } else {
            0.0
        };

        results.push(json!({
            "question": question,
            "good_count": good,
            "bad_count": bad,
            "bad_rate": bad_rate,
        }));
    }

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!(results)),
        None,
    )
}

/// GET /api/admin/questions/{id}
/// 問題の詳細と投票情報を返す
pub async fn question_detail(
    _admin: AdminClaims,
    Path(path): Path<QuestionPath>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    // 問題を取得
    let question = match db.read::<Question>("questions", &path.id).await {
        Ok(Some(q)) => q,
        Ok(None) => {
            return response_handler(
                StatusCode::NOT_FOUND,
                "error".to_string(),
                None,
                Some("question not found".to_string()),
            );
        }
        Err(e) => {
            error!("Failed to fetch question: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e),
            );
        }
    };

    // この問題への投票を取得
    let votes = match db
        .client
        .fluent()
        .list()
        .from("votes")
        .obj::<Vote>()
        .stream_all()
        .await
    {
        Ok(stream) => stream.collect::<Vec<Vote>>().await,
        Err(e) => {
            error!("Failed to fetch votes: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e.to_string()),
            );
        }
    };

    let good = votes.iter().filter(|v| v.parent_id == path.id && v.vote == "good").count();
    let bad = votes.iter().filter(|v| v.parent_id == path.id && v.vote == "bad").count();

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({
            "question": question,
            "votes": {
                "good": good,
                "bad": bad,
            }
        })),
        None,
    )
}

/// DELETE /api/admin/questions/{id}
/// 問題を削除する
pub async fn delete_question(
    _admin: AdminClaims,
    Path(path): Path<QuestionPath>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    match db.delete("questions", &path.id).await {
        Ok(_) => response_handler(
            StatusCode::OK,
            "success".to_string(),
            Some(json!({ "deleted": path.id })),
            None,
        ),
        Err(e) => {
            error!("Failed to delete question: {}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e),
            )
        }
    }
}
