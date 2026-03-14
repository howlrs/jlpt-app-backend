use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
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

/// GET /api/admin/stats
/// レベル・カテゴリごとの統計情報を返す
pub async fn stats(
    _admin: AdminClaims,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    // 全問題を取得
    let questions: Vec<Question> = match db
        .client
        .fluent()
        .list()
        .from("questions")
        .obj::<Question>()
        .stream_all()
        .await
    {
        Ok(stream) => stream.collect::<Vec<Question>>().await,
        Err(e) => {
            error!("Failed to fetch questions: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e.to_string()),
            );
        }
    };

    // 全投票を取得
    let votes: Vec<Vote> = match db
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

    // question_id -> level_id のマッピングを作成
    let mut question_level: HashMap<String, u32> = HashMap::new();
    for q in &questions {
        question_level.insert(q.id.clone(), q.level_id);
    }

    // レベルごとの投票集計
    let mut level_votes: HashMap<u32, (i64, i64)> = HashMap::new();
    for v in &votes {
        if let Some(&lid) = question_level.get(&v.parent_id) {
            let entry = level_votes.entry(lid).or_insert((0, 0));
            if v.vote == "good" {
                entry.0 += 1;
            } else if v.vote == "bad" {
                entry.1 += 1;
            }
        }
    }

    // レベル・カテゴリごとに集計
    // key: level_id -> (level_name, HashMap<category_name, (questions, sub_questions)>)
    let mut level_map: HashMap<u32, (String, HashMap<String, (usize, usize)>)> = HashMap::new();
    for q in &questions {
        let entry = level_map
            .entry(q.level_id)
            .or_insert_with(|| (q.level_name.clone(), HashMap::new()));
        let cat = entry.1.entry(q.category_name.clone()).or_insert((0, 0));
        cat.0 += 1;
        cat.1 += q.sub_questions.len();
    }

    // レスポンス構築
    let mut levels: Vec<serde_json::Value> = level_map
        .into_iter()
        .map(|(level_id, (level_name, categories))| {
            let total_questions: usize = categories.values().map(|(q, _)| q).sum();
            let total_sub_questions: usize = categories.values().map(|(_, s)| s).sum();
            let (good_votes, bad_votes) = level_votes.get(&level_id).copied().unwrap_or((0, 0));

            let mut cats: Vec<serde_json::Value> = categories
                .into_iter()
                .map(|(name, (questions, sub_questions))| {
                    json!({
                        "name": name,
                        "questions": questions,
                        "sub_questions": sub_questions,
                    })
                })
                .collect();
            cats.sort_by(|a, b| {
                a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
            });

            json!({
                "level_id": level_id,
                "level_name": level_name,
                "total_questions": total_questions,
                "total_sub_questions": total_sub_questions,
                "good_votes": good_votes,
                "bad_votes": bad_votes,
                "categories": cats,
            })
        })
        .collect();

    levels.sort_by(|a, b| {
        a["level_id"].as_u64().unwrap_or(0).cmp(&b["level_id"].as_u64().unwrap_or(0))
    });

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({ "levels": levels })),
        None,
    )
}

/// GET /api/admin/coverage-stats
/// カテゴリ別カバレッジ統計を返す
pub async fn coverage_stats(
    _admin: AdminClaims,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    // 全問題を取得
    let questions: Vec<Question> = match db
        .client
        .fluent()
        .list()
        .from("questions")
        .obj::<Question>()
        .stream_all()
        .await
    {
        Ok(stream) => stream.collect::<Vec<Question>>().await,
        Err(e) => {
            error!("Failed to fetch questions: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e.to_string()),
            );
        }
    };

    // レベル・カテゴリごとに集計
    // key: (level_id) -> (level_name, HashMap<(category_id, category_name), (question_count, sub_question_count)>)
    let mut level_map: HashMap<u32, (String, HashMap<(String, String), (usize, usize)>)> =
        HashMap::new();
    for q in &questions {
        let entry = level_map
            .entry(q.level_id)
            .or_insert_with(|| (q.level_name.clone(), HashMap::new()));
        let cat_id = q.category_id.clone().unwrap_or_default();
        let cat = entry
            .1
            .entry((cat_id, q.category_name.clone()))
            .or_insert((0, 0));
        cat.0 += 1;
        cat.1 += q.sub_questions.len();
    }

    // ターゲット決定関数
    fn determine_target(category_name: &str) -> usize {
        if category_name.contains("文法") {
            300
        } else if category_name.contains("読解") || category_name.contains("内容理解") {
            200
        } else if category_name.contains("聴解")
            || category_name.contains("課題理解")
            || category_name.contains("ポイント")
            || category_name.contains("概要")
        {
            150
        } else {
            100
        }
    }

    // レスポンス構築
    let mut levels: Vec<serde_json::Value> = level_map
        .into_iter()
        .map(|(level_id, (level_name, categories))| {
            let total_questions: usize = categories.values().map(|(q, _)| q).sum();

            let mut cats: Vec<serde_json::Value> = categories
                .into_iter()
                .map(|((cat_id, cat_name), (question_count, sub_question_count))| {
                    let target = determine_target(&cat_name);
                    let coverage_pct = if target > 0 {
                        (sub_question_count as f64 / target as f64) * 100.0
                    } else {
                        0.0
                    };
                    json!({
                        "category_id": cat_id,
                        "category_name": cat_name,
                        "question_count": question_count,
                        "sub_question_count": sub_question_count,
                        "target": target,
                        "coverage_pct": (coverage_pct * 10.0).round() / 10.0,
                    })
                })
                .collect();
            cats.sort_by(|a, b| {
                a["category_id"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["category_id"].as_str().unwrap_or(""))
            });

            json!({
                "level_id": level_id,
                "level_name": level_name,
                "total_questions": total_questions,
                "categories": cats,
            })
        })
        .collect();

    levels.sort_by(|a, b| {
        a["level_id"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&b["level_id"].as_u64().unwrap_or(0))
    });

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({ "levels": levels })),
        None,
    )
}

#[derive(Deserialize)]
pub struct BulkDeleteRequest {
    ids: Vec<String>,
}

/// POST /api/admin/questions/bulk-delete
/// 問題を一括削除する
pub async fn bulk_delete(
    _admin: AdminClaims,
    State(db): State<Arc<crate::common::database::Database>>,
    Json(body): Json<BulkDeleteRequest>,
) -> impl IntoResponse {
    let mut deleted: usize = 0;
    let mut failed: usize = 0;

    for id in &body.ids {
        match db.delete("questions", id).await {
            Ok(_) => deleted += 1,
            Err(e) => {
                error!("Failed to delete question {}: {}", id, e);
                failed += 1;
            }
        }
    }

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({
            "deleted": deleted,
            "failed": failed,
        })),
        None,
    )
}
