use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use log::info;
use serde::Deserialize;
use serde_json::json;

use crate::{api::utils::response_handler, models::evaluate::Vote};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoteType {
    Good,
    Bad,
}

impl std::fmt::Display for VoteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoteType::Good => write!(f, "good"),
            VoteType::Bad => write!(f, "bad"),
        }
    }
}

#[derive(Deserialize)]
pub struct PathParams {
    vote: VoteType,
}

#[derive(Deserialize)]
pub struct QueryParams {
    parent_id: Option<String>,
    child_id: Option<String>,
}

pub async fn vote(
    Path(path_params): Path<PathParams>,
    Query(query_params): Query<QueryParams>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    let parent_id = query_params.parent_id.as_deref().unwrap_or_default();
    let child_id = query_params.child_id.as_deref().unwrap_or_default();

    // parent_id, child_id の長さ制限
    if parent_id.len() > 128 || child_id.len() > 128 {
        return response_handler(
            StatusCode::BAD_REQUEST,
            "error".to_string(),
            None,
            Some("IDが長すぎます".to_string()),
        );
    }

    let vote_str = path_params.vote.to_string();

    info!(
        "vote: {}, parent_id: {}, child_id: {}",
        vote_str, parent_id, child_id
    );

    let vote = Vote::new(
        vote_str.clone(),
        Some("questions".to_string()),
        parent_id.to_string(),
        child_id.to_string(),
    );

    match db
        .client
        .fluent()
        .insert()
        .into("votes")
        .document_id(vote.id())
        .object(&vote)
        .execute::<Vote>()
        .await
    {
        Ok(_) => response_handler(
            StatusCode::OK,
            "success".to_string(),
            Some(json!({
                "vote": &vote_str,
                "parent_id": parent_id,
                "child_id": child_id,
            })),
            None,
        ),
        Err(e) => response_handler(
            StatusCode::INTERNAL_SERVER_ERROR,
            "error".to_string(),
            None,
            Some(e.to_string()),
        ),
    }
}
