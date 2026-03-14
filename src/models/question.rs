use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Serialize, Debug, Default)]
pub struct Question {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub level_id: u32,
    #[serde(default)]
    pub level_name: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub category_name: String,

    #[serde(default)]
    pub sentence: String,
    pub prerequisites: Option<String>,
    pub sub_questions: Vec<SubQuestion>,

    /// 生成に使用したAIモデル名（品質追跡用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
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
            #[serde(default)]
            level_name: String,
            #[serde(default)]
            category_id: Option<CategoryId>,
            #[serde(default)]
            category_name: String,
            #[serde(default)]
            sentence: String,
            prerequisites: Option<String>,
            #[serde(default)]
            sub_questions: Vec<SubQuestion>,
            #[serde(default)]
            generated_by: Option<String>,
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
            level_id: helper.level_id,
            level_name: helper.level_name,
            category_id,
            category_name: helper.category_name,
            sentence: helper.sentence,
            prerequisites: helper.prerequisites,
            sub_questions: helper.sub_questions,
            generated_by: helper.generated_by,
        })
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SelectAnswer {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SubQuestion {
    #[serde(default)]
    pub id: u32,

    pub sentence: Option<String>,
    pub prerequisites: Option<String>,
    pub select_answer: Vec<SelectAnswer>,
    pub answer: String,
}
