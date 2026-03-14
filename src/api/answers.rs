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
use uuid::Uuid;

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

    let user_answer = UserAnswer {
        id: Uuid::new_v4().to_string(),
        user_id: claims.user_id.clone(),
        question_id: body.question_id.clone(),
        sub_question_id: body.sub_question_id,
        level_id: question.level_id,
        category_name: question.category_name.clone(),
        selected_answer: body.selected_answer.clone(),
        correct_answer,
        is_correct,
        answered_at: chrono::Utc::now().timestamp(),
    };

    let doc_id = user_answer.id.clone();
    match db
        .create::<UserAnswer>("user_answers", &doc_id, user_answer.clone())
        .await
    {
        Ok(_) => response_handler(
            StatusCode::OK,
            "success".to_string(),
            Some(json!(user_answer)),
            None,
        ),
        Err(e) => {
            error!("Failed to save user_answer: {}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(e),
            )
        }
    }
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
            let mut results = Vec::new();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(answer) => results.push(answer),
                    Err(e) => {
                        error!("Error reading user_answer: {:?}", e);
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
    // Query all user_answers for this user
    match db
        .client
        .fluent()
        .select()
        .from("user_answers")
        .filter(|q| {
            q.field(path!(UserAnswer::user_id)).eq(claims.user_id.clone())
        })
        .obj::<UserAnswer>()
        .stream_query_with_errors()
        .await
    {
        Ok(mut stream) => {
            let mut answers: Vec<UserAnswer> = Vec::new();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(answer) => answers.push(answer),
                    Err(e) => {
                        error!("Error reading user_answer: {:?}", e);
                    }
                }
            }

            let total_answered = answers.len() as u32;
            let total_correct = answers.iter().filter(|a| a.is_correct).count() as u32;
            let accuracy = if total_answered > 0 {
                total_correct as f64 / total_answered as f64
            } else {
                0.0
            };

            // Aggregate by level_id and category_name
            use std::collections::BTreeMap;

            struct LevelStats {
                total: u32,
                correct: u32,
                categories: BTreeMap<String, (u32, u32)>, // (total, correct)
            }

            let mut levels: BTreeMap<u32, LevelStats> = BTreeMap::new();

            for answer in &answers {
                let level = levels.entry(answer.level_id).or_insert_with(|| LevelStats {
                    total: 0,
                    correct: 0,
                    categories: BTreeMap::new(),
                });
                level.total += 1;
                if answer.is_correct {
                    level.correct += 1;
                }
                let cat = level
                    .categories
                    .entry(answer.category_name.clone())
                    .or_insert((0, 0));
                cat.0 += 1;
                if answer.is_correct {
                    cat.1 += 1;
                }
            }

            let levels_json: Vec<serde_json::Value> = levels
                .iter()
                .map(|(level_id, stats)| {
                    let level_name = format!("N{}", level_id);
                    let cat_accuracy = if stats.total > 0 {
                        stats.correct as f64 / stats.total as f64
                    } else {
                        0.0
                    };
                    let categories: Vec<serde_json::Value> = stats
                        .categories
                        .iter()
                        .map(|(name, (t, c))| {
                            let a = if *t > 0 { *c as f64 / *t as f64 } else { 0.0 };
                            json!({
                                "name": name,
                                "total": t,
                                "correct": c,
                                "accuracy": a,
                            })
                        })
                        .collect();
                    json!({
                        "level_id": level_id,
                        "level_name": level_name,
                        "total": stats.total,
                        "correct": stats.correct,
                        "accuracy": cat_accuracy,
                        "categories": categories,
                    })
                })
                .collect();

            response_handler(
                StatusCode::OK,
                "success".to_string(),
                Some(json!({
                    "total_answered": total_answered,
                    "total_correct": total_correct,
                    "accuracy": accuracy,
                    "levels": levels_json,
                })),
                None,
            )
        }
        Err(e) => {
            error!("Failed to query stats: {:?}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(format!("{:?}", e)),
            )
        }
    }
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
