use std::sync::Arc;

use axum::{
    Router,
    http::Method,
    routing::{get, post},
};
use log::info;
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

    let origin = vec![
        std::env::var("FRONTEND_URL")
            .unwrap_or("https://storage.googleapis.com".to_string())
            .parse()
            .unwrap(),
    ];

    let db = Arc::new(common::database::Database::new().await);

    let endpoint = Router::new()
        // サーバー時間を返すエンドポイント
        .route("/api/health", get(api::initial::health))
        .route(
            // レベル及びカテゴリ一の取得エンドポイン
            "/api/meta",
            get(api::meta::get),
        )
        .route(
            // 問題取得エンドポイント
            // レベル・カテゴリ必須
            // 複合インデックスを利用しているため、レベル・カテゴリの組み合わせが存在しない場合は空配列を返す
            // カテゴリ内問題をランダムで取得
            "/api/level/{level_id}/categories/{category_id}/questions",
            get(api::question::get),
        )
        .route(
            // 問題に対する評価
            "/api/evaluate/{vote}",
            get(api::evaluate::vote),
        )
        .route("/api/signup", post(api::user::signup))
        .route("/api/signin", post(api::user::signin))
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
                // JSON でのリクエストを許可
                .allow_headers(["Content-Type".parse().unwrap()]),
        )
        .with_state(db);

    // Access-Control-Allow-Origin: *

    let port = std::env::var("PORT").unwrap_or("8080".to_string());
    let lister = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    axum::serve::serve(lister, endpoint).await.unwrap();
}
