use axum::{
    Router,
    http::Method,
    routing::{get, post},
};
use log::info;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::api::{category, evaluate, initial, meta, question};

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

    let db = common::database::Database::new().await;

    let endpoint = Router::new()
        .route(
            // レベル及びカテゴリ一の取得エンドポイント
            "/api/meta",
            get(meta::get),
        )
        .route(
            // 問題取得エンドポイント
            // レベル・カテゴリ必須
            // 複合インデックスを利用しているため、レベル・カテゴリの組み合わせが存在しない場合は空配列を返す
            // カテゴリ内問題をランダムで取得
            "/api/level/{level_id}/categories/{category_id}/questions",
            get(question::get),
        )
        .route(
            // 問題に対する評価
            "/api/evaluate/{vote}",
            get(evaluate::vote),
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
