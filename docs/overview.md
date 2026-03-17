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

JLPT（日本語能力試験）対策学習アプリのバックエンドAPI。問題の配信、ユーザー認証、学習履歴管理、問題評価機能を提供する。

## 責務

1. JLPT問題・回答データの提供（レベル別・カテゴリ別、複合インデックス最適化済み）
2. ユーザー認証（サインアップ / サインイン / httpOnly Cookie JWT）
3. 学習履歴管理（回答記録・不正解履歴・カテゴリ別正答率統計）
4. メタデータ配信（レベル一覧・カテゴリ一覧）
5. 問題評価（Good/Bad 投票）
6. 問題品質の自動監視・異常検出・削除（Cloud Scheduler週次実行）
7. 品質レポートのDiscord通知

## 主要技術スタック

| 技術 | 用途 |
|------|------|
| **Axum** | Webフレームワーク・ルーティング |
| **Firestore** | NoSQLドキュメントDB |
| **jsonwebtoken** | JWT認証（httpOnly Cookie） |
| **Argon2** | パスワードハッシュ化 |
| **tower-http** | CORS・セキュリティヘッダー |
| **tower_governor** | レート制限（SmartIpKeyExtractor） |
| **Tokio** | 非同期ランタイム |
| **Serde** | JSON シリアライゼーション |

## セキュリティ

- httpOnly Cookie認証（localStorage完全廃止）
- レート制限（signin/signup: 5バースト/IP、evaluate: 10バースト/IP）
- セキュリティヘッダー（X-Frame-Options, X-Content-Type-Options, HSTS）
- 入力バリデーション（メール形式・パスワード強度・vote enum型）
- ユーザー列挙防止（統一エラーメッセージ + タイミング均一化）
- CORS（単一オリジン + allow_credentials）

## 関連リポジトリ

| リポジトリ | 役割 |
|-----------|------|
| [japanese-app](../../japanese-app/docs/) | 問題生成スクリプト（初期版） |
| [jlpt-app-scripts](../../jlpt-app-scripts/docs/) | データ加工・DB投入パイプライン |
