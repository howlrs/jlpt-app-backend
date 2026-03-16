use std::sync::{Arc, LazyLock};

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use log::error;
use serde::Deserialize;
use serde_json::{self, json};
use uuid::Uuid;

use crate::{
    api::utils::response_handler,
    models::claim::{hash_password, verify_password, Claims},
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

/// httpOnly Cookie を構築する
fn build_auth_cookie(token: &str) -> Cookie<'static> {
    let is_production = std::env::var("FRONTEND_URL")
        .map(|u| u.starts_with("https://"))
        .unwrap_or(false);

    let mut cookie = Cookie::build(("access_token", token.to_string()))
        .http_only(true)
        .same_site(SameSite::None)
        .path("/api")
        .max_age(time::Duration::hours(24));

    if is_production {
        cookie = cookie.secure(true);
    }

    cookie.build()
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
    jar: CookieJar,
    Json(req): Json<SigninRequest>,
) -> impl IntoResponse {
    // メールアドレスバリデーション
    if !is_valid_email(&req.email) {
        let _ = verify_password(&DUMMY_HASH, &req.password);
        return (
            jar,
            response_handler(
                StatusCode::UNAUTHORIZED,
                "error".to_string(),
                None,
                Some(AUTH_ERROR_MSG.to_string()),
            ),
        );
    }

    // emailでユーザーを検索
    let db_user = match db
        .read::<crate::models::user::User>("users", &req.email)
        .await
    {
        Ok(user) => user,
        Err(_) => {
            let _ = verify_password(&DUMMY_HASH, &req.password);
            return (
                jar,
                response_handler(
                    StatusCode::UNAUTHORIZED,
                    "error".to_string(),
                    None,
                    Some(AUTH_ERROR_MSG.to_string()),
                ),
            );
        }
    };

    let db_user = match db_user {
        Some(user) => user,
        None => {
            let _ = verify_password(&DUMMY_HASH, &req.password);
            return (
                jar,
                response_handler(
                    StatusCode::UNAUTHORIZED,
                    "error".to_string(),
                    None,
                    Some(AUTH_ERROR_MSG.to_string()),
                ),
            );
        }
    };

    // パスワードの検証
    match verify_password(&db_user.password, &req.password) {
        Ok(true) => {}
        Ok(false) => {
            return (
                jar,
                response_handler(
                    StatusCode::UNAUTHORIZED,
                    "error".to_string(),
                    None,
                    Some(AUTH_ERROR_MSG.to_string()),
                ),
            );
        }
        Err(e) => {
            error!("パスワード検証エラー: {}", e);
            return (
                jar,
                response_handler(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "error".to_string(),
                    None,
                    Some("認証処理に失敗しました".to_string()),
                ),
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
    let claims = Claims::new(user.user_id.clone(), user.email.clone(), role);

    let to_token = match claims.to_token() {
        Ok(token) => token,
        Err(e) => {
            error!("token creation error: {:?}", e);
            return (
                jar,
                response_handler(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "error".to_string(),
                    None,
                    Some("認証処理に失敗しました".to_string()),
                ),
            );
        }
    };

    // httpOnly Cookie を設定
    let cookie = build_auth_cookie(&to_token);
    let jar = jar.add(cookie);

    (
        jar,
        response_handler(
            StatusCode::OK,
            "success".to_string(),
            Some(json!({
                "user": json!(user),
            })),
            None,
        ),
    )
}

/// GET /api/auth/me — 認証状態を確認
pub async fn auth_me(claims: Claims) -> impl IntoResponse {
    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({
            "user_id": claims.user_id,
            "email": claims.email,
            "role": claims.role,
        })),
        None,
    )
}

/// POST /api/auth/logout — Cookie クリア
pub async fn auth_logout(jar: CookieJar) -> impl IntoResponse {
    let cookie = Cookie::build(("access_token", ""))
        .http_only(true)
        .same_site(SameSite::None)
        .path("/api")
        .max_age(time::Duration::seconds(0))
        .build();
    let jar = jar.add(cookie);
    (
        jar,
        response_handler(StatusCode::OK, "success".to_string(), None, None),
    )
}
