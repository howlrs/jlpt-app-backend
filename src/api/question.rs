use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use firestore::{FirestoreQueryDirection, path};
use log::info;
use serde::Deserialize;
use serde_json::json;

use tokio_stream::StreamExt;

use crate::{api::utils::response_handler, models::question::Question};

#[derive(Deserialize)]
pub struct PathParams {
    level_id: u32,
    category_id: u32,
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
    State(db): State<crate::common::database::Database>,
) -> impl IntoResponse {
    // level_idを受けて、そのレベルに紐づくカテゴリー群を取得する
    info!(
        "level_id: {}, category_id: {}, limit: {}",
        path_params.level_id,
        path_params.category_id,
        query_params.limit.unwrap_or_default()
    );

    // [TODO] ランダムで問題を返す方法を決めかねている
    // 案1: v4であるドキュメントIDに対して比較で取得
    // 案2: レベル・カテゴリ指定のため、1000前後の取得になる。それを指定配列長でランダムに取得
    // 案3: 0<1でランダムな浮動小数点を持つフィールドを追加し、複合インデックスを作成し、乱数で範囲を取得する

    // レスポンスを返す
    // [TODO] 現状DESCENDINGで取得しているためいつも同じ問題が出力される
    let mut questions = read_db(&path_params, db).await;
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

    info!("limit: {}", questions.len());

    response_handler(
        StatusCode::OK,
        "ok".to_string(),
        Some(json!(questions)),
        None,
    )
}

async fn read_db(path_params: &PathParams, db: crate::common::database::Database) -> Vec<Question> {
    match db
        .client
        .fluent()
        .select()
        .from("questions")
        .filter(|x| {
            x.for_all([
                x.field(path!(Question::level_id)).eq(path_params.level_id),
                x.field(path!(Question::category_id))
                    .eq(path_params.category_id),
            ])
        })
        // .limit(200)
        .order_by([(path!(Question::id), FirestoreQueryDirection::Descending)])
        .obj::<Question>()
        .stream_query_with_errors()
        .await
    {
        Ok(mut data) => {
            let mut result = Vec::new();
            while let Some(item) = data.next().await {
                match item {
                    Ok(item) => result.push(item),
                    Err(e) => eprintln!("Error: {:?}", e),
                }
            }
            result
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
            vec![]
        }
    }
}
