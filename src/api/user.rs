use std::sync::{Arc, LazyLock};

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};

use log::error;
use serde::Deserialize;
use serde_json::{self, json};
use uuid::Uuid;

use crate::{
    api::utils::response_handler,
    models::claim::{hash_password, verify_password},
};

/// ダミーハッシュ: ユーザー未存在時のタイミング攻撃防止用
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    hash_password("dummy_password_for_timing_safety").expect("Failed to create dummy hash")
});

/// 統一認証エラーメッセージ
const AUTH_ERROR_MSG: &str = "メールアドレスまたはパスワードが正しくありません";

#[derive(Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct SigninRequest {
    pub email: String,
    pub password: String,
}

/// メールアドレスの簡易バリデーション
fn is_valid_email(email: &str) -> bool {
    let at_pos = email.find('@');
    let dot_pos = email.rfind('.');
    match (at_pos, dot_pos) {
        (Some(at), Some(dot)) => {
            at > 0 && dot > at + 1 && dot < email.len() - 1 && email.len() <= 254
        }
        _ => false,
    }
}

pub async fn signup(
    State(db): State<Arc<crate::common::database::Database>>,
    Json(req): Json<SignupRequest>,
) -> impl IntoResponse {
    // メールアドレスバリデーション
    if !is_valid_email(&req.email) {
        return response_handler(
            StatusCode::BAD_REQUEST,
            "error".to_string(),
            None,
            Some("有効なメールアドレスを入力してください".to_string()),
        );
    }

    // パスワード強度チェック
    if req.password.len() < 8 || req.password.len() > 128 {
        return response_handler(
            StatusCode::BAD_REQUEST,
            "error".to_string(),
            None,
            Some("パスワードは8〜128文字で入力してください".to_string()),
        );
    }

    let mut user = crate::models::user::User::new();
    user.id = Uuid::now_v7().to_string();
    user.email = req.email.clone();
    user.password = match hash_password(&req.password) {
        Ok(hashed) => hashed,
        Err(e) => {
            error!("パスワードハッシュ化エラー: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some("登録処理に失敗しました".to_string()),
            );
        }
    };
    user.created_at = Some(chrono::Utc::now());

    // ユーザー情報をDBに登録
    let key = user.email.clone();
    match db
        .create::<crate::models::user::User>("users", key.as_str(), user)
        .await
    {
        Ok(_) => response_handler(StatusCode::OK, "success".to_string(), None, None),
        Err(e) => {
            error!("create to database error: {:?}", e);
            response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some("登録処理に失敗しました".to_string()),
            )
        }
    }
}

pub async fn signin(
    State(db): State<Arc<crate::common::database::Database>>,
    Json(req): Json<SigninRequest>,
) -> impl IntoResponse {
    // メールアドレスバリデーション
    if !is_valid_email(&req.email) {
        // ダミーハッシュ比較でタイミングを均一化
        let _ = verify_password(&DUMMY_HASH, &req.password);
        return response_handler(
            StatusCode::UNAUTHORIZED,
            "error".to_string(),
            None,
            Some(AUTH_ERROR_MSG.to_string()),
        );
    }

    // emailでユーザーを検索
    let db_user = match db
        .read::<crate::models::user::User>("users", &req.email)
        .await
    {
        Ok(user) => user,
        Err(_) => {
            // ダミーハッシュ比較でタイミングを均一化
            let _ = verify_password(&DUMMY_HASH, &req.password);
            return response_handler(
                StatusCode::UNAUTHORIZED,
                "error".to_string(),
                None,
                Some(AUTH_ERROR_MSG.to_string()),
            );
        }
    };

    let db_user = match db_user {
        Some(user) => user,
        None => {
            // ダミーハッシュ比較でタイミングを均一化
            let _ = verify_password(&DUMMY_HASH, &req.password);
            return response_handler(
                StatusCode::UNAUTHORIZED,
                "error".to_string(),
                None,
                Some(AUTH_ERROR_MSG.to_string()),
            );
        }
    };

    // パスワードの検証
    match verify_password(&db_user.password, &req.password) {
        Ok(true) => {}
        Ok(false) => {
            return response_handler(
                StatusCode::UNAUTHORIZED,
                "error".to_string(),
                None,
                Some(AUTH_ERROR_MSG.to_string()),
            );
        }
        Err(e) => {
            error!("パスワード検証エラー: {}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some("認証処理に失敗しました".to_string()),
            );
        }
    }

    // データベースの値でユーザー情報を構築
    let mut user = crate::models::user::User::new();
    user.email = req.email.clone();
    user.merge_with(db_user);

    // Admin権限の判定
    let role = {
        let admin_emails = std::env::var("ADMIN_EMAILS").unwrap_or_default();
        let is_admin = admin_emails
            .split(',')
            .map(|e| e.trim())
            .any(|e| e == user.email);
        if is_admin {
            Some("admin".to_string())
        } else {
            None
        }
    };

    // クレーム発行
    let claims = crate::models::claim::Claims::new(user.user_id.clone(), user.email.clone(), role);

    let to_token = match claims.to_token() {
        Ok(token) => token,
        Err(e) => {
            error!("token creation error: {:?}", e);
            return response_handler(
                StatusCode::INTERNAL_SERVER_ERROR,
                "error".to_string(),
                None,
                Some("認証処理に失敗しました".to_string()),
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
