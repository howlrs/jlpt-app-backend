use std::{env, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use firestore::path;
use log::info;
use serde::Deserialize;
use serde_json::json;

use crate::{api::utils::response_handler, models::question::Question};

#[derive(Deserialize)]
pub struct PathParams {
    level_id: u32,
    category_id: u32,
}

#[derive(Deserialize)]
pub struct LevelPathParams {
    level_id: u32,
}

#[derive(Deserialize)]
pub struct QueryParams {
    limit: Option<u32>,
}

/// # get
///
/// ## 概要
/// レベル内カテゴリ内の問題を取得
///
/// ## HTTP情報
/// - **メソッド**: GET
/// - **パス**: /api/levels/{level_id: u32}/categories/{category_id: u32}/questions
/// - **認証**: 不要
///
/// ## パラメータ
/// - `level_id`: レベルID (u32) - レベルを指定するID
/// - `category_id`: カテゴリID (u32) - カテゴリを指定するID
///
/// ## クエリ
/// - `limit`: 取得する問題数 (u32) - 取得する問題数を指定する
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
///     "data": [Quesion{}...]
///   }
///   ```
///
/// ### エラー時
/// - **ステータスコード**: 404 Not Found
/// - **内容**: リソースが存在しない場合のエラーメッセージ
///
/// ## 例
/// GET /api/levels/1/categories/1/questions
///
/// ## 関連エンドポイント
/// - `get_answer`: 回答取得エンドポイント
/// - `get_hint`: ヒント取得エンドポイント
pub async fn get(
    Path(path_params): Path<PathParams>,
    Query(query_params): Query<QueryParams>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    // level_idを受けて、そのレベルに紐づくカテゴリー群を取得する
    info!(
        "level_id: {}, category_id: {}, limit: {}",
        path_params.level_id,
        path_params.category_id,
        query_params.limit.unwrap_or_default()
    );

    // 全問題を取得し、limitが指定されていればシャッフルして指定数だけ返す（案2を採用）
    let mut questions = read_db(&path_params, db.clone()).await;
    if questions.is_empty() {
        return response_handler(
            StatusCode::NOT_FOUND,
            "Not Found".to_string(),
            None,
            Some(
                format!(
                    "database has not questions, level_id: {}, category_id: {}",
                    path_params.level_id, path_params.category_id
                )
                .to_string(),
            ),
        );
    }

    info!(
        "level_id: {}, category_id: {} -> db has length: {}",
        path_params.level_id,
        path_params.category_id,
        questions.len()
    );

    // limitがあれば、指定数だけ取得
    let questions = match query_params.limit {
        Some(limit) => {
            if questions.len() < limit as usize {
                questions
            } else {
                use rand::seq::SliceRandom;
                let mut rng = rand::rng();
                questions.shuffle(&mut rng);
                questions.into_iter().take(limit as usize).collect()
            }
        }
        None => questions,
    };

    info!("result count: {}", questions.len());

    response_handler(
        StatusCode::OK,
        "ok".to_string(),
        Some(json!(questions)),
        None,
    )
}

/// GET /api/level/{level_id}/questions
pub async fn get_by_level(
    Path(path_params): Path<LevelPathParams>,
    Query(query_params): Query<QueryParams>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    info!(
        "level_id: {}, limit: {}",
        path_params.level_id,
        query_params.limit.unwrap_or_default()
    );

    let mut questions = read_level_db(&path_params, db.clone()).await;
    if questions.is_empty() {
        return response_handler(
            StatusCode::NOT_FOUND,
            "Not Found".to_string(),
            None,
            Some(format!(
                "database has not questions, level_id: {}",
                path_params.level_id
            )),
        );
    }

    use rand::seq::SliceRandom;
    let mut rng = rand::rng();
    questions.shuffle(&mut rng);

    let questions = match query_params.limit {
        Some(limit) => questions.into_iter().take(limit as usize).collect(),
        None => questions,
    };

    info!(
        "level_id: {} -> random result count: {}",
        path_params.level_id,
        questions.len()
    );

    response_handler(
        StatusCode::OK,
        "ok".to_string(),
        Some(json!(questions)),
        None,
    )
}

/// GET /api/questions/{id}
pub async fn get_by_id(
    Path(id): Path<String>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    match db.read::<Question>("questions", &id).await {
        Ok(Some(q)) => response_handler(StatusCode::OK, "ok".to_string(), Some(json!(q)), None),
        Ok(None) => response_handler(
            StatusCode::NOT_FOUND,
            "Not Found".to_string(),
            None,
            Some("question not found".to_string()),
        ),
        Err(e) => response_handler(
            StatusCode::INTERNAL_SERVER_ERROR,
            "error".to_string(),
            None,
            Some(e),
        ),
    }
}

fn is_active_question(question: &Question, active_dataset: Option<&str>) -> bool {
    let quality_ok = !matches!(
        question.quality_status.as_deref(),
        Some("quarantine" | "needs_human_review")
    );
    let dataset_ok = match active_dataset {
        Some("legacy") => question.dataset.as_deref().unwrap_or("legacy") == "legacy",
        Some(dataset) => question.dataset.as_deref() == Some(dataset),
        None => question.dataset.as_deref().unwrap_or("legacy") == "legacy",
    };
    quality_ok && dataset_ok
}

async fn read_db(
    path_params: &PathParams,
    db: Arc<crate::common::database::Database>,
) -> Vec<Question> {
    let cat_id_str = path_params.category_id.to_string();
    let active_dataset = env::var("QUESTION_DATASET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // 複合インデックス (level_id + category_id) を使用してFirestore側でフィルタ
    match db
        .client
        .fluent()
        .select()
        .from("questions")
        .filter(|x| {
            x.for_all([
                x.field(path!(Question::level_id)).eq(path_params.level_id),
                x.field(path!(Question::category_id)).eq(&cat_id_str),
            ])
        })
        .obj::<Question>()
        .query()
        .await
    {
        Ok(data) => {
            let active: Vec<Question> = data
                .into_iter()
                .filter(|q| is_active_question(q, active_dataset.as_deref()))
                .collect();
            info!(
                "Firestore returned {} active questions for N{}/cat={} dataset={}",
                active.len(),
                path_params.level_id,
                cat_id_str,
                active_dataset.as_deref().unwrap_or("legacy")
            );
            active
        }
        Err(e) => {
            log::error!("Question query error: {:?}", e);
            vec![]
        }
    }
}

async fn read_level_db(
    path_params: &LevelPathParams,
    db: Arc<crate::common::database::Database>,
) -> Vec<Question> {
    let active_dataset = env::var("QUESTION_DATASET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    match db
        .client
        .fluent()
        .select()
        .from("questions")
        .filter(|q| q.field(path!(Question::level_id)).eq(path_params.level_id))
        .obj::<Question>()
        .query()
        .await
    {
        Ok(data) => {
            let active: Vec<Question> = data
                .into_iter()
                .filter(|q| is_active_question(q, active_dataset.as_deref()))
                .collect();
            info!(
                "Firestore returned {} active questions for N{} dataset={}",
                active.len(),
                path_params.level_id,
                active_dataset.as_deref().unwrap_or("legacy")
            );
            active
        }
        Err(e) => {
            log::error!("Level question query error: {:?}", e);
            vec![]
        }
    }
}
