use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    api::utils::{self, response_handler},
    models::meta::{CatValue, Meta, Value},
};

/// # get
///
/// ## 概要
/// レベル・カテゴリ一覧を取得
/// [TODO] メタデータに問題数を追加する
///
/// ## HTTP情報
/// - **メソッド**: GET
/// - **パス**: /api/meta
/// - **認証**: 不要
///
/// ## パラメータ
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
///     "data": [{
///         "id": 1, // カテゴリID
///        "level_id": u32, // レベルID
///         "name": "category_name"
///     }...]
///   }
///   ```
///
/// ### エラー時
/// - **ステータスコード**: 404 Not Found
/// - **内容**: リソースが存在しない場合のエラーメッセージ
///
/// ## 例
/// GET /api/levels/1/categories
///
/// ## 関連エンドポイント
/// - `create`: カテゴリ作成エンドポイント
pub async fn get(State(db): State<Arc<crate::common::database::Database>>) -> impl IntoResponse {
    let levels = db
        .read_all::<Value>("levels", None)
        .await
        .unwrap_or_default();

    let mut categories = db
        .read_all::<CatValue>("categories", None)
        .await
        .unwrap_or_default();

    if categories.is_empty() || levels.is_empty() {
        return response_handler(
            StatusCode::NOT_FOUND,
            "error".to_string(),
            None,
            Some("meta data not found".to_string()),
        );
    }

    // sort by name length
    categories.sort_by_key(|category| utils::kanji_len(&category.name));

    let meta = Meta { levels, categories };

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!(meta)),
        None,
    )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewValue {
    pub id: u32,
    pub name: String,
    pub reten: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewCatValue {
    pub level_id: u32,
    pub id: u32,
    pub name: String,
    pub reten: u32,
}

#[cfg(test)]
mod tests {

    use std::{path::PathBuf, vec};

    use firestore::path;
    use serde_json::Value;
    use tokio_stream::StreamExt;

    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct NewCatValue {
        pub level_id: u32,
        pub id: u32,
        pub name: String,
        pub reten: Option<u32>,
    }

    /// メタデータのカテゴリに問題数を追加する
    #[tokio::test]
    async fn test_renew_categories() {
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let target_env = PathBuf::from(crate_dir).join(".env.local");
        dotenv::from_filename(target_env).ok();

        let db = crate::common::database::Database::new().await;

        // CatValuesを取得
        let categories = db
            .read_all::<CatValue>("categories_meta", None)
            .await
            .unwrap_or_default();

        let mut new_categories = vec![];
        categories.iter().for_each(|category| {
            new_categories.push(NewCatValue {
                level_id: category.level_id,
                id: category.id,
                name: category.name.clone(),
                reten: None,
            });
        });

        // 各レベルxカテゴリの問題数を取得
        for category in new_categories.iter_mut() {
            if let Some(count) = category.reten {
                if count > 0 {
                    println!(
                        "level: {}, category: {} - count: {}",
                        category.level_id, category.id, count
                    );
                    continue;
                }
            }

            let count = match db
                .client
                .fluent()
                .select()
                .fields(&[path!(crate::models::question::Question::id)])
                .from("questions")
                .filter(|x| {
                    x.for_all([
                        x.field(path!(crate::models::question::Question::level_id))
                            .eq(category.level_id),
                        x.field(path!(crate::models::question::Question::category_id))
                            .eq(category.id),
                    ])
                })
                .obj::<Value>()
                .stream_query_with_errors()
                .await
            {
                Ok(mut data) => {
                    let mut count = 0;

                    while let Some(item) = data.next().await {
                        match item {
                            Ok(_) => {
                                count += 1;
                            }
                            Err(e) => {
                                eprintln!("error: {:?}", e);
                            }
                        }
                    }
                    count
                }
                Err(e) => {
                    eprintln!("error: {:?}", e);
                    0
                }
            };

            // CatValue型に問題数を追加
            category.reten = Some(count);
            println!(
                "level: {}, category: {} - count: {}",
                category.level_id, category.id, count
            );
        }

        // レベルをデータベースに保存
        for category in new_categories.iter() {
            if let Some(count) = category.reten {
                if count < 1 {
                    println!(
                        "reten is none, level: {}, category: {} - count: {}",
                        category.level_id, category.id, count
                    );
                    continue;
                }
            }

            let uid = uuid::Uuid::new_v4().to_string();

            match db
                .client
                .fluent()
                .insert()
                .into("categories")
                .document_id(uid)
                .object(category)
                .execute::<NewCatValue>()
                .await
            {
                Ok(_) => {
                    println!("success: {:?}", category);
                }
                Err(e) => {
                    eprintln!("error: {:?}", e);
                }
            }
        }
    }
}
