use serde::{Deserialize, Serialize};

/// ユーザによる問題の「報告」(誤字・選択肢不備・内容不正など)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionReport {
    pub question_id: String,
    pub user_id: String,
    pub reported_at: i64,
}

impl QuestionReport {
    pub fn new(question_id: String, user_id: String) -> Self {
        Self {
            question_id,
            user_id,
            reported_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 同じユーザによる同じ問題への二重報告を防ぐため、合成キーを doc id に使う
    pub fn doc_id(question_id: &str, user_id: &str) -> String {
        format!("{}_{}", question_id, user_id)
    }
}
