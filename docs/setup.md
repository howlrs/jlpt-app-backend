# jlpt-app-backend セットアップガイド

## 前提条件

- Rust (Edition 2024 対応版)
- Google Cloud プロジェクト（Firestore有効化済み）
- Docker（デプロイ時）

## 環境変数

`.env.local` ファイルを作成：

| 変数名 | 必須 | 説明 | デフォルト |
|--------|------|------|-----------|
| `PROJECT_ID` | Yes | Google Cloud プロジェクトID | - |
| `JWT_SECRET` | Yes | JWT署名用シークレットキー | - |
| `FRONTEND_URL` | No | CORS許可オリジン | `https://jlpt.howlrs.net` |
| `PORT` | No | サーバーポート | 8080 |
| `ADMIN_EMAILS` | No | 管理者メールアドレス（カンマ区切り） | - |

> **注意:** サインアップは全てのユーザーに開放されています。`ADMIN_EMAILS` は管理者ロールの制御のみに使用されます。

## ローカル開発

```bash
# ビルド
cargo build

# 実行
cargo run
```

サーバーが `http://0.0.0.0:8080` で起動する。

## デプロイ（Google Cloud Run）

```bash
gcloud run deploy backend --source .
```

Dockerfileによるマルチステージビルドが実行される：
1. `rust:1.85.0-slim` ベースイメージでリリースビルド
2. コンパイル済みバイナリを実行

## ディレクトリ構成

```
src/
├── main.rs           # エントリポイント（ルーター設定、CORS、DB接続）
├── api/              # APIハンドラー
│   ├── initial.rs    # ヘルスチェック
│   ├── meta.rs       # レベル・カテゴリメタデータ
│   ├── question.rs   # 問題取得
│   ├── user.rs       # ユーザー認証
│   ├── evaluate.rs   # 問題評価
│   └── utils.rs      # レスポンスユーティリティ
├── models/           # データモデル
│   ├── question.rs   # Question / SubQuestion
│   ├── user.rs       # User
│   ├── claim.rs      # JWT Claims
│   ├── evaluate.rs   # Vote
│   └── meta.rs       # Level / Category
└── common/           # 共通モジュール
    └── database.rs   # Firestore CRUD ラッパー
```

## 備考

- パスワード関連関数（ハッシュ化・検証）は `Result` を返す設計に変更済み（パニックしない）

## Firestore インデックス

以下の複合インデックスが必要：

- コレクション `questions`: `level_id` (ASC) + `category_id` (ASC)
