use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    // 関連する情報
    pub levels: Vec<Value>,
    pub categories: Vec<CatValue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Value {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CatValue {
    pub level_id: u32,
    pub id: u32,
    pub name: String,
}
