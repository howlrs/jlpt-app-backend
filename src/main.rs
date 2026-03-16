use std::sync::Arc;

use axum::{
    Router,
    http::Method,
    routing::{get, post},
};
use log::{error, info};
use tower_http::cors::CorsLayer;

mod api;
mod common;
mod models;

#[tokio::main]
async fn main() {
    match dotenv::from_filename(".env.local") {
        Ok(_) => info!("Found .env.local"),
        Err(_) => info!("Not found .env.local"),
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| {
        error!("FRONTEND_URL未設定。デフォルト: https://jlpt.howlrs.net");
        "https://jlpt.howlrs.net".to_string()
    });

    let origin = vec![frontend_url.parse().unwrap_or_else(|e| {
        error!("FRONTEND_URLのパース失敗: {} - デフォルト使用", e);
        "https://jlpt.howlrs.net".parse().unwrap()
    })];

    let db = Arc::new(common::database::Database::new().await);

    let endpoint = Router::new()
        .route("/api/public/health", get(api::initial::public_health))
        .route("/api/private/health", get(api::initial::private_health))
        .route("/api/meta", get(api::meta::get))
        .route(
            "/api/level/{level_id}/categories/{category_id}/questions",
            get(api::question::get),
        )
        .route("/api/questions/{id}", get(api::question::get_by_id))
        .route("/api/evaluate/{vote}", get(api::evaluate::vote))
        .route("/api/signup", post(api::user::signup))
        .route("/api/signin", post(api::user::signin))
        .route("/api/answers", post(api::answers::record_answer))
        .route("/api/users/me/history", get(api::answers::history))
        .route("/api/users/me/stats", get(api::answers::stats))
        .route("/api/users/me/mistakes", get(api::answers::mistakes))
        .route("/api/admin/votes/summary", get(api::admin::votes_summary))
        .route("/api/admin/questions/bad", get(api::admin::bad_questions))
        .route("/api/admin/stats", get(api::admin::stats))
        .route("/api/admin/coverage-stats", get(api::admin::coverage_stats))
        .route(
            "/api/admin/questions/bulk-delete",
            post(api::admin::bulk_delete),
        )
        .route(
            "/api/admin/questions/{id}",
            get(api::admin::question_detail).delete(api::admin::delete_question),
        )
        .route(
            "/api/admin/monitor-quality",
            post(api::monitor::monitor_quality),
        )
        .layer(
            CorsLayer::new()
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_origin(origin)
                .allow_headers([
                    "Content-Type".parse().unwrap(),
                    "Authorization".parse().unwrap(),
                ]),
        )
        .with_state(db);

    let port = std::env::var("PORT").unwrap_or("8080".to_string());
    info!("サーバー起動: 0.0.0.0:{}", port);

    let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
        Ok(l) => l,
        Err(e) => {
            error!("ポート{}のバインドに失敗: {}", port, e);
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve::serve(listener, endpoint).await {
        error!("サーバーエラー: {}", e);
        std::process::exit(1);
    }
}
