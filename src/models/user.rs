use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct User {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub password: String,
}
