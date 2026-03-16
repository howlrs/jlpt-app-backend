use std::{fmt::Display, sync::LazyLock};

use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json, RequestPartsExt,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};

use serde::{Deserialize, Serialize};
use serde_json::json;

// Axum examples/jwt 実装を踏襲
// https://github.com/tokio-rs/axum/blob/main/examples/jwt/src/main.rs

pub static JWT_SECRET: LazyLock<String> =
    LazyLock::new(|| std::env::var("JWT_SECRET").expect("JWT_SECRET must be set"));

static KEYS: LazyLock<Keys> = LazyLock::new(|| Keys::new(JWT_SECRET.as_bytes()));

pub enum AuthError {
    InvalidToken,
    MissingToken,
    Forbidden,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            AuthError::MissingToken => (StatusCode::BAD_REQUEST, "Missing token"),
            AuthError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),
        };

        let body = Json(json!({
            "code": status.as_u16(),
            "error": message.to_string(),
        }));

        (status, body).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: String,
    pub email: String,
    pub exp: i64,
    pub role: Option<String>,
}

impl Claims {
    pub fn new(user_id: String, email: String, role: Option<String>) -> Self {
        let after24h = chrono::Utc::now().timestamp() + 60 * 60 * 24;
        Self {
            user_id,
            email,
            exp: after24h,
            role,
        }
    }

    pub fn is_ok(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.exp > now
    }

    pub fn to_token(&self) -> Result<String, String> {
        encode(&Header::default(), self, &KEYS.encoding).map_err(|e| e.to_string())
    }
}

// Claim Display
impl Display for Claims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let now = chrono::Utc::now();
        let one_week = chrono::Duration::weeks(1);
        let after_oneweek = now + one_week;

        write!(
            f,
            "Claims: user_id: {}, email: {}, exp: {}",
            self.user_id,
            self.email,
            after_oneweek.format("%Y-%m-%d %H:%M")
        )
    }
}

// BearerトークンからClaimsを抽出する関数
pub async fn extract_bearer_token(
    TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>,
) -> Result<Claims, AuthError> {
    validate_jwt(bearer.token()).map_err(|_| AuthError::InvalidToken)
}

// JWTトークン検証関数
fn validate_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

// Axum Extract — Cookie優先、Authorization ヘッダーフォールバック
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = AuthError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // 1. Cookie から access_token を取得（優先）
        let jar = parts.extract::<axum_extra::extract::CookieJar>().await.unwrap();
        if let Some(cookie) = jar.get("access_token") {
            return validate_jwt(cookie.value()).map_err(|_| AuthError::InvalidToken);
        }

        // 2. Authorization ヘッダーからBearerトークンを取得（フォールバック）
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AuthError::InvalidToken)?;
        validate_jwt(bearer.token()).map_err(|_| AuthError::InvalidToken)
    }
}

// Admin権限を持つClaimsを抽出するExtractor
#[derive(Debug)]
pub struct AdminClaims(pub Claims);

impl<S> FromRequestParts<S> for AdminClaims
where
    S: Send + Sync,
{
    type Rejection = AuthError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let claims = Claims::from_request_parts(parts, state).await?;
        match &claims.role {
            Some(role) if role == "admin" => Ok(AdminClaims(claims)),
            _ => Err(AuthError::Forbidden),
        }
    }
}

pub struct Keys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl Keys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

/// パスワード文字列のHash化
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    argon
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| format!("パスワードハッシュ化失敗: {}", e))
}

/// パスワード文字列の検証
pub fn verify_password(hashed_password: &str, password: &str) -> Result<bool, String> {
    let parsed_hash = argon2::PasswordHash::new(hashed_password)
        .map_err(|e| format!("ハッシュパース失敗: {}", e))?;
    let argon = Argon2::default();
    Ok(argon
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
// 暗号化された文字列と復元した文字列を出力し比較する
mod tests {

    use super::*;

    #[test]
    fn test_token() {
        let password = "password";
        let hashed_password = hash_password(password).unwrap();
        println!("Hashed password, {} -> {}", password, hashed_password);
        assert!(verify_password(&hashed_password, password).unwrap());
    }
}
