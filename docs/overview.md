# jlpt-app-backend 概要

## プロジェクト情報

| 項目 | 内容 |
|------|------|
| 言語 | Rust (Edition 2024) |
| フレームワーク | Axum 0.8.1 |
| データベース | Google Firestore |
| デプロイ先 | Google Cloud Run |
| 本番URL | https://jlpt.howlrs.net/ |

## 概要

JLPT（日本語能力試験）対策学習アプリのバックエンドAPI。問題の配信、ユーザー認証、問題評価機能を提供する。

## 責務

1. JLPT問題・回答データの提供（レベル別・カテゴリ別）
2. ユーザー認証（サインアップ / サインイン / JWT）
3. メタデータ配信（レベル一覧・カテゴリ一覧）
4. 問題評価（Good/Bad 投票）
5. 問題品質の自動監視・異常検出・削除（Cloud Scheduler週次実行）
6. 品質レポートのDiscord通知

## 主要技術スタック

| 技術 | 用途 |
|------|------|
| **Axum** | Webフレームワーク・ルーティング |
| **Firestore** | NoSQLドキュメントDB |
| **jsonwebtoken** | JWT認証 |
| **Argon2** | パスワードハッシュ化 |
| **tower-http** | CORSミドルウェア |
| **Tokio** | 非同期ランタイム |
| **Serde** | JSON シリアライゼーション |

## 関連リポジトリ

| リポジトリ | 役割 |
|-----------|------|
| [japanese-app](../../japanese-app/docs/) | 問題生成スクリプト（初期版） |
| [jlpt-app-scripts](../../jlpt-app-scripts/docs/) | データ加工・DB投入パイプライン |
