use axum::{extract::State, http::StatusCode, response::IntoResponse};
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
pub async fn get(State(db): State<crate::common::database::Database>) -> impl IntoResponse {
    let levels = db
        .read_all::<Value>("levels", None)
        .await
        .unwrap_or_default();

    // [TODO] カテゴリに属する問題数を取得する
    let mut categories = db
        .read_all::<CatValue>("categories_meta", None)
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
