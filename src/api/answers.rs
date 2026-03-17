use std::sync::Arc;

use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use firestore::{path, FirestoreQueryDirection};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_stream::StreamExt;

use crate::{
    api::utils::response_handler,
    models::claim::Claims,
    models::question::Question,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAnswer {
    pub id: String,
    pub user_id: String,
    pub question_id: String,
    pub sub_question_id: u32,
    pub level_id: u32,
    pub category_name: String,
    pub selected_answer: String,
    pub correct_answer: String,
    pub is_correct: bool,
    pub answered_at: i64,
}

const MAX_USER_ANSWERS: u32 = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStatsEntry {
    pub total: u32,
    pub correct: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelStatsEntry {
    pub total: u32,
    pub correct: u32,
    pub categories: std::collections::HashMap<String, CategoryStatsEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStatsDoc {
    pub user_id: String,
    pub total_answers: u32,
    pub total_correct: u32,
    pub levels: std::collections::HashMap<String, LevelStatsEntry>,
}

#[derive(Debug, Deserialize)]
pub struct RecordAnswerRequest {
    pub question_id: String,
    pub sub_question_id: u32,
    pub selected_answer: String,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<u32>,
}

/// POST /api/answers
pub async fn record_answer(
    claims: Claims,
    State(db): State<Arc<crate::common::database::Database>>,
    Json(body): Json<RecordAnswerRequest>,
) -> impl IntoResponse {
    // Fetch the question from Firestore
    let question: Question = match db.read::<Question>("questions", &body.question_id).await {
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

    // Find the sub_question
    let sub_question = match question
        .sub_questions
        .iter()
        .find(|sq| sq.id == body.sub_question_id)
    {
        Some(sq) => sq,
        None => {
            return response_handler(
                StatusCode::NOT_FOUND,
                "error".to_string(),
                None,
                Some("sub_question not found".to_string()),
            );
        }
    };

    let correct_answer = sub_question.answer.clone();
    let is_correct = body.selected_answer == correct_answer;
    let level_key = format!("N{}", question.level_id);

    // 1) Update user_stats incrementally
    let stats_id = claims.user_id.clone();
    let mut user_stats: UserStatsDoc = match db.read::<UserStatsDoc>("user_stats", &stats_id).await {
        Ok(Some(s)) => s,
        _ => UserStatsDoc {
            user_id: claims.user_id.clone(),
            total_answers: 0,
            total_correct: 0,
            levels: std::collections::HashMap::new(),
        },
    };
    user_stats.total_answers += 1;
    if is_correct {
        user_stats.total_correct += 1;
    }
    let level_entry = user_stats.levels.entry(level_key).or_insert_with(|| LevelStatsEntry {
        total: 0,
        correct: 0,
        categories: std::collections::HashMap::new(),
    });
    level_entry.total += 1;
    if is_correct {
        level_entry.correct += 1;
    }
    let cat_entry = level_entry.categories.entry(question.category_name.clone()).or_insert_with(|| CategoryStatsEntry {
        total: 0,
        correct: 0,
    });
    cat_entry.total += 1;
    if is_correct {
        cat_entry.correct += 1;
    }

    if let Err(e) = db.update::<UserStatsDoc>("user_stats", &stats_id, user_stats.clone()).await {
        // If update fails (doc doesn't exist yet), try create
        if let Err(e2) = db.create::<UserStatsDoc>("user_stats", &stats_id, user_stats.clone()).await {
            error!("Failed to save user_stats: {} / {}", e, e2);
        }
    }

    // 2) Save to user_answers only if incorrect (upsert: 同じ問題の重複を防止)
    if !is_correct {
        // 決定的ID: user_id + question_id + sub_question_id で一意に特定
        let doc_id = format!(
            "{}_{}_{}", claims.user_id, body.question_id, body.sub_question_id
        );

        let user_answer = UserAnswer {
            id: doc_id.clone(),
            user_id: claims.user_id.clone(),
            question_id: body.question_id.clone(),
            sub_question_id: body.sub_question_id,
            level_id: question.level_id,
            category_name: question.category_name.clone(),
            selected_answer: body.selected_answer.clone(),
            correct_answer,
            is_correct: false,
            answered_at: chrono::Utc::now().timestamp(),
        };

        // upsert: 既存なら上書き、なければ作成
        if let Err(e) = db.update::<UserAnswer>("user_answers", &doc_id, user_answer.clone()).await {
            if let Err(e2) = db.create::<UserAnswer>("user_answers", &doc_id, user_answer).await {
                error!("Failed to save user_answer: {} / {}", e, e2);
            }
        }

        // 3) Prune old answers if over limit
        if let Ok(mut stream) = db
            .client
            .fluent()
            .select()
            .from("user_answers")
            .filter(|q| {
                q.field(path!(UserAnswer::user_id)).eq(claims.user_id.clone())
            })
            .order_by([(
                path!(UserAnswer::answered_at),
                FirestoreQueryDirection::Descending,
            )])
            .obj::<UserAnswer>()
            .stream_query_with_errors()
            .await
        {
            let mut all_answers = Vec::new();
            while let Some(item) = stream.next().await {
                if let Ok(a) = item {
                    all_answers.push(a);
                }
            }
            if all_answers.len() as u32 > MAX_USER_ANSWERS {
                for old in &all_answers[MAX_USER_ANSWERS as usize..] {
                    let _ = db.delete("user_answers", &old.id).await;
                }
            }
        }
    }

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({ "is_correct": is_correct })),
        None,
    )
}

/// GET /api/users/me/history?limit=50
pub async fn history(
    claims: Claims,
    Query(query): Query<HistoryQuery>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);

    match db
        .client
        .fluent()
        .select()
        .from("user_answers")
        .filter(|q| {
            q.field(path!(UserAnswer::user_id)).eq(claims.user_id.clone())
        })
        .order_by([(
            path!(UserAnswer::answered_at),
            FirestoreQueryDirection::Descending,
        )])
        .limit(limit)
        .obj::<UserAnswer>()
        .stream_query_with_errors()
        .await
    {
        Ok(mut stream) => {
            let mut answers = Vec::new();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(answer) => answers.push(answer),
                    Err(e) => {
                        error!("Error reading user_answer: {:?}", e);
                    }
                }
            }

            // question_id で重複除外（最新のみ保持）
            let mut seen = std::collections::HashSet::new();
            let results: Vec<serde_json::Value> = answers.iter().filter_map(|a| {
                if !seen.insert(a.question_id.clone()) {
                    return None;
                }
                let level_name = format!("N{}", a.level_id);
                let level_slug = format!("n{}", a.level_id);
                let created_at = chrono::DateTime::from_timestamp(a.answered_at, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default();
                Some(json!({
                    "id": a.id,
                    "question_id": a.question_id,
                    "level_name": level_name,
                    "level_slug": level_slug,
                    "category_name": a.category_name,
                    "created_at": created_at,
                }))
            }).collect();

            response_handler(
                StatusCode::OK,
                "success".to_string(),
                Some(json!(results)),
                None,
            )
        }
        Err(e) => {
            error!("Failed to query history: {:?}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(format!("{:?}", e)),
            )
        }
    }
}

/// GET /api/users/me/stats
pub async fn stats(
    claims: Claims,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    let user_stats: UserStatsDoc = match db.read::<UserStatsDoc>("user_stats", &claims.user_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return response_handler(
                StatusCode::OK,
                "success".to_string(),
                Some(json!({
                    "total_answers": 0,
                    "total_correct": 0,
                    "overall_accuracy": 0.0,
                    "levels": [],
                })),
                None,
            );
        }
        Err(e) => {
            error!("Failed to read user_stats: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e),
            );
        }
    };

    let overall_accuracy = if user_stats.total_answers > 0 {
        user_stats.total_correct as f64 / user_stats.total_answers as f64 * 100.0
    } else {
        0.0
    };

    let mut levels_json: Vec<serde_json::Value> = user_stats
        .levels
        .iter()
        .map(|(level_name, level)| {
            let level_accuracy = if level.total > 0 {
                level.correct as f64 / level.total as f64 * 100.0
            } else {
                0.0
            };
            let categories: Vec<serde_json::Value> = level
                .categories
                .iter()
                .map(|(cat_name, cat)| {
                    let cat_accuracy = if cat.total > 0 {
                        cat.correct as f64 / cat.total as f64 * 100.0
                    } else {
                        0.0
                    };
                    json!({
                        "category_name": cat_name,
                        "total": cat.total,
                        "correct": cat.correct,
                        "accuracy": cat_accuracy,
                    })
                })
                .collect();
            json!({
                "level_name": level_name,
                "total": level.total,
                "correct": level.correct,
                "accuracy": level_accuracy,
                "categories": categories,
            })
        })
        .collect();

    // Sort by level name (N1, N2, ...)
    levels_json.sort_by(|a, b| {
        a["level_name"].as_str().unwrap_or("").cmp(&b["level_name"].as_str().unwrap_or(""))
    });

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({
            "total_answers": user_stats.total_answers,
            "total_correct": user_stats.total_correct,
            "overall_accuracy": overall_accuracy,
            "levels": levels_json,
        })),
        None,
    )
}

/// GET /api/users/me/mistakes?limit=20
pub async fn mistakes(
    claims: Claims,
    Query(query): Query<HistoryQuery>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20);

    match db
        .client
        .fluent()
        .select()
        .from("user_answers")
        .filter(|q| {
            q.for_all([
                q.field(path!(UserAnswer::user_id)).eq(claims.user_id.clone()),
                q.field(path!(UserAnswer::is_correct)).eq(false),
            ])
        })
        .order_by([(
            path!(UserAnswer::answered_at),
            FirestoreQueryDirection::Descending,
        )])
        .limit(limit)
        .obj::<UserAnswer>()
        .stream_query_with_errors()
        .await
    {
        Ok(mut stream) => {
            let mut results = Vec::new();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(answer) => results.push(answer),
                    Err(e) => {
                        error!("Error reading mistake: {:?}", e);
                    }
                }
            }
            response_handler(
                StatusCode::OK,
                "success".to_string(),
                Some(json!(results)),
                None,
            )
        }
        Err(e) => {
            error!("Failed to query mistakes: {:?}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(format!("{:?}", e)),
            )
        }
    }
}
