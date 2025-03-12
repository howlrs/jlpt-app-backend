use std::sync::Arc;

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};

use log::error;
use serde_json::{self, json};
use uuid::Uuid;

use crate::{
    api::utils::response_handler,
    models::claim::{hash_password, verify_password},
};

pub async fn signup(
    State(db): State<Arc<crate::common::database::Database>>,
    Json(v): Json<serde_json::Value>,
) -> impl IntoResponse {
    let user = match serde_json::from_value::<crate::models::user::User>(v) {
        Ok(mut user) => {
            // パスワードのハッシュ化
            // 登録日ソート可能なUUIDを生成
            // Userに紐づくデータのキーに使用する
            user.id = Uuid::now_v7().to_string();
            user.password = hash_password(user.password.as_str());
            user.created_at = Some(chrono::Utc::now());
            user
        }
        Err(e) => {
            error!("serde_json::from_value error: {:?}", e);
            return response_handler(
                StatusCode::BAD_REQUEST,
                "error".to_string(),
                None,
                Some(e.to_string()),
            );
        }
    };

    // ユーザー情報をDBに登録
    let key = user.email.clone();
    match db
        .create::<crate::models::user::User>("users", key.as_str(), user)
        .await
    {
        Ok(_) => response_handler(StatusCode::OK, "success".to_string(), None, None),
        Err(e) => {
            error!("crate to database error: {:?}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(format!("{:?}", e)),
            )
        }
    }
}

pub async fn signin(
    State(db): State<Arc<crate::common::database::Database>>,
    Json(v): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut user = match serde_json::from_value::<crate::models::user::User>(v) {
        Ok(user) => user,
        Err(e) => {
            error!("serde_json::from_value error: {:?}", e);
            return response_handler(
                StatusCode::BAD_REQUEST,
                "error".to_string(),
                None,
                Some(e.to_string()),
            );
        }
    };

    // [TODO] ログイン用の検証
    // - emailでユーザーを検索
    let result = match db
        .read::<crate::models::user::User>("users", &user.email)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            error!("not found user, {:?}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(format!("not found user, {:?}", e)),
            );
        }
    };
    // - パスワードの検証
    if let Some(db_user) = result.clone() {
        if !verify_password(&db_user.password, &user.password) {
            return response_handler(
                StatusCode::UNAUTHORIZED,
                "error".to_string(),
                None,
                Some("wrong password".to_string()),
            );
        }

        // データベースの値を代入
        user.merge_with(db_user);
    } else {
        error!("not found user");
        return response_handler(
            StatusCode::UNAUTHORIZED,
            "error".to_string(),
            None,
            Some("not found user".to_string()),
        );
    }

    // クレーム発行
    let claims = crate::models::claim::Claims::new(user.user_id.clone(), user.email.clone());

    let to_token = match claims.to_token() {
        Ok(token) => token,
        Err(e) => {
            error!("token creation error: {:?}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some(format!("{:?}", e)),
            );
        }
    };

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({
            "token": to_token,
            "token_type": "bearer",
            "user": json!(user),
        })),
        None,
    )
}
