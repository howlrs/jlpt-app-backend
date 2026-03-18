use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    pub token_hash: String,
    pub user_id: String,
    pub token_family: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub is_revoked: bool,
    pub revoked_at: Option<i64>,
}

impl RefreshToken {
    pub fn new(raw_token: &str, user_id: String, token_family: String) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            token_hash: Self::hash(raw_token),
            user_id,
            token_family,
            issued_at: now,
            expires_at: now + 14 * 24 * 60 * 60, // 14 days
            is_revoked: false,
            revoked_at: None,
        }
    }

    pub fn hash(raw_token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(raw_token.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp() > self.expires_at
    }

    pub fn generate_raw_token() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}
