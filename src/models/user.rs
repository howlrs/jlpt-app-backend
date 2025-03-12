use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct User {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub password: String,
    pub ip: Option<String>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

impl User {
    pub fn new() -> Self {
        User {
            id: "".to_string(),
            user_id: "".to_string(),
            email: "".to_string(),
            password: "".to_string(),
            ip: None,
            language: None,
            country: None,
            created_at: None,
        }
    }

    // self, Userを引数に取り、両者を比較し、selfが不足している値を受け取る
    pub fn merge_with(&mut self, user: User) -> User {
        if self.id.is_empty() {
            self.id = user.id;
        } else if self.user_id.is_empty() {
            self.user_id = user.user_id;
        } else if self.email.is_empty() {
            self.email = user.email;
        } else if self.ip.is_none() {
            self.ip = user.ip;
        } else if self.language.is_none() {
            self.language = user.language;
        } else if self.country.is_none() {
            self.country = user.country;
        } else if self.created_at.is_none() {
            self.created_at = user.created_at;
        }

        self.password = "".to_string();

        self.clone()
    }
}
