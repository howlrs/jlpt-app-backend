use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

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

#[derive(Clone, Serialize, Debug, Default)]
pub struct Question {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub level_id: Option<u32>,
    pub level_name: Option<String>,
    #[serde(default)]
    pub category_id: Option<u32>,
    pub category_name: Option<String>,

    pub chapter: Option<String>,
    pub sentence: Option<String>,
    pub prerequisites: Option<String>,
    pub sub_questions: Vec<SubQuestion>,
}

// QuestionのDeserializeトレイトの実装を拡張
impl<'de> Deserialize<'de> for Question {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct QuestionHelper {
            #[serde(default)]
            id: Option<String>,
            #[serde(default)]
            level_id: u32,
            level_name: String,
            #[serde(default)]
            category_id: Option<CategoryId>,
            category_name: String,
            #[serde(default)]
            chapter: String,
            sentence: String,
            prerequisites: Option<String>,
            sub_questions: Vec<SubQuestion>,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CategoryId {
            String(String),
            Number(u32),
        }

        let helper = QuestionHelper::deserialize(deserializer)?;

        let category_id = match helper.category_id {
            Some(CategoryId::String(s)) => Some(s),
            Some(CategoryId::Number(n)) => Some(n.to_string()),
            None => None,
        };

        Ok(Question {
            id: helper.id.unwrap_or_default(),
            level_id: Some(helper.level_id),
            level_name: Some(helper.level_name),
            category_id: Some(category_id.unwrap().parse().unwrap()),
            category_name: Some(helper.category_name),
            chapter: Some(helper.chapter),
            sentence: Some(helper.sentence),
            prerequisites: helper.prerequisites,
            sub_questions: helper.sub_questions,
        })
    }
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
