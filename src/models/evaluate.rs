use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Vote {
    pub id: String,
    pub vote: String,
    pub where_to: Option<String>,
    pub parent_id: String,
    pub child_id: String,
    pub created_at: i64,
}

impl Vote {
    pub fn new(
        vote: String,
        where_to: Option<String>,
        parent_id: String,
        child_id: String,
    ) -> Self {
        let uid = Uuid::new_v4().to_string();
        Self {
            id: uid,
            vote,
            where_to,
            parent_id,
            child_id,
            created_at: Utc::now().timestamp(),
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }
}
