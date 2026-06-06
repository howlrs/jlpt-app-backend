use std::sync::Arc;

use axum::extract::FromRef;
use tokio::sync::OnceCell;

use crate::{common::database::Database, models::meta::Meta};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub meta_cache: Arc<OnceCell<Meta>>,
}

impl AppState {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            meta_cache: Arc::new(OnceCell::new()),
        }
    }
}

impl FromRef<AppState> for Arc<Database> {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
