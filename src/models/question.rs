use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub id: u32,
    // 関連する情報
    pub level_id: u32,
    pub category_id: u32,
    pub hint_id: u32,
    pub answer_id: u32,

    pub name: String,
    pub title: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Question {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub level_id: u32,
    pub level_name: Option<String>,
    #[serde(default)]
    pub category_id: u32,
    pub category_name: Option<String>,

    pub chapter: Option<String>,
    pub sentence: Option<String>,
    pub prerequisites: Option<String>,
    pub sub_questions: Vec<SubQuestion>,
}

type SelectAnswer = HashMap<String, String>;

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SubQuestion {
    #[serde(default)]
    pub id: u32,
    #[serde(default)]
    pub hint_id: u32,
    #[serde(default)]
    pub answer_id: u32,

    pub sentence: Option<String>,
    pub prerequisites: Option<String>,
    pub select_answer: Vec<SelectAnswer>,
    pub answer: String,
}
