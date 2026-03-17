# jlpt-app-backend API仕様

## 認証方式

| 項目 | 内容 |
|------|------|
| トークン形式 | JWT (HS256) |
| 送受信方式 | httpOnly Cookie (`access_token`) |
| Cookie属性 | `HttpOnly; Secure; SameSite=Lax; Path=/api; Max-Age=86400` |
| 有効期限 | 24時間 |
| Claims | `{ user_id, email, exp, role }` |
| パスワードハッシュ | Argon2id |
| 管理者権限 | `ADMIN_EMAILS` 環境変数でメール一致判定 |

認証が必要なエンドポイントは、Cookie → Authorization ヘッダー（Bearer）の順でトークンを検索する（デュアルサポート）。

## セキュリティ

| 機能 | 詳細 |
|------|------|
| レート制限 | signin/signup: 5バースト/IP, evaluate: 10バースト/IP (tower_governor + SmartIpKeyExtractor) |
| セキュリティヘッダー | X-Frame-Options: DENY, X-Content-Type-Options: nosniff, Strict-Transport-Security |
| 入力バリデーション | メール形式、パスワード8〜128文字、vote enum型 (`good`/`bad` のみ) |
| ユーザー列挙防止 | 統一エラーメッセージ + ダミーArgon2比較（タイミング均一化） |
| CORS | 単一オリジン (FRONTEND_URL) + allow_credentials(true) |

## エンドポイント一覧

### パブリックAPI（認証不要）

#### `GET /api/public/health`

ヘルスチェック。

**レスポンス:** `200 OK`
```json
{
  "message": "success",
  "data": { "health": "ok" }
}
```

---

#### `GET /api/meta`

全レベル・カテゴリのメタデータを取得。

**レスポンス:** `200 OK`
```json
{
  "message": "success",
  "data": {
    "levels": [{ "id": 1, "name": "N1" }],
    "categories": [{ "level_id": 1, "id": 1, "name": "語彙", "reten": 50 }]
  }
}
```

---

#### `GET /api/level/{level_id}/categories/{category_id}/questions`

指定レベル・カテゴリの問題を取得。Firestore複合インデックス（`level_id` + `category_id`）を使用。

**パスパラメータ:**

| パラメータ | 型 | 説明 |
|-----------|-----|------|
| `level_id` | u32 | JLPTレベル (1-5) |
| `category_id` | u32 | カテゴリID |

**クエリパラメータ:**

| パラメータ | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| `limit` | u32 | No | 取得件数上限（指定時はランダム順） |

**レスポンス:** `200 OK`

**エラー:** `404 Not Found`

---

#### `GET /api/questions/{id}`

問題を1件取得。

**レスポンス:** `200 OK` / `404 Not Found`

---

#### `GET /api/evaluate/{vote}`

問題に対する評価を記録。レート制限あり（10バースト/IP）。

**パスパラメータ:**

| パラメータ | 型 | 説明 |
|-----------|-----|------|
| `vote` | enum | `good` または `bad`（それ以外は400エラー） |

**クエリパラメータ:**

| パラメータ | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| `parent_id` | String | No | 問題ID（最大128文字） |
| `child_id` | String | No | 小問ID（最大128文字） |

---

#### `POST /api/signup`

ユーザー登録。レート制限あり（5バースト/IP）。

**リクエストボディ:**
```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

**バリデーション:**
- メール: 形式チェック（@, ドット, 最大254文字）
- パスワード: 8〜128文字

**レスポンス:** `200 OK`

**エラー:** `400 Bad Request`（バリデーション失敗） / `500 Internal Server Error`

---

### 認証API（認証不要 / Cookie設定）

#### `POST /api/signin`

ユーザーログイン。成功時に httpOnly Cookie を設定。レート制限あり（5バースト/IP）。

**リクエストボディ:**
```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

**成功レスポンス:** `200 OK` + `Set-Cookie: access_token=<JWT>`
```json
{
  "message": "success",
  "data": {
    "user_id": "019ceb10-...",
    "email": "user@example.com",
    "role": "admin"
  }
}
```

`role` は管理者の場合のみ `"admin"`、一般ユーザーは `null`。

**エラー:** `401 Unauthorized`（統一メッセージ: 「メールアドレスまたはパスワードが正しくありません」）

---

#### `GET /api/auth/me`

Cookie内のJWTを検証し、認証状態を返す。

**認証:** Cookie (access_token)

**レスポンス:** `200 OK`
```json
{
  "message": "success",
  "data": {
    "user_id": "019ceb10-...",
    "email": "user@example.com",
    "role": "admin"
  }
}
```

**エラー:** `401 Unauthorized`

---

#### `POST /api/auth/logout`

Cookieをクリアしてログアウト。

**レスポンス:** `200 OK` + `Set-Cookie: access_token=; Max-Age=0`

---

### ユーザーAPI（Cookie認証必須）

#### `POST /api/answers`

ユーザーの回答を記録。不正解の場合は `user_answers` に保存（同一問題は上書き）、`user_stats` を更新。

**リクエストボディ:**
```json
{
  "question_id": "uuid",
  "sub_question_id": 1,
  "selected_answer": "2"
}
```

**レスポンス:** `200 OK`
```json
{
  "message": "success",
  "data": { "is_correct": false }
}
```

---

#### `GET /api/users/me/history?limit=50`

不正解の回答履歴を取得。`question_id` で重複除外し、最新のみ返却。

**クエリパラメータ:**

| パラメータ | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| `limit` | u32 | No | 取得件数上限（デフォルト: 50） |

---

#### `GET /api/users/me/stats`

カテゴリ別正答率統計を取得。

**レスポンス:** `200 OK`
```json
{
  "message": "success",
  "data": {
    "total_answers": 100,
    "total_correct": 75,
    "overall_accuracy": 75.0,
    "levels": [
      {
        "level_name": "N3",
        "total": 50,
        "correct": 40,
        "accuracy": 80.0,
        "categories": [
          { "category_name": "文法", "total": 20, "correct": 15, "accuracy": 75.0 }
        ]
      }
    ]
  }
}
```

---

#### `GET /api/users/me/mistakes?limit=20`

不正解回答の詳細一覧を取得。

---

### 管理者API（Cookie認証 + 管理者権限必須）

#### `GET /api/admin/votes/summary`
投票サマリー。

#### `GET /api/admin/questions/bad`
低品質問題一覧。

#### `GET /api/admin/stats`
レベル別統計。

#### `GET /api/admin/coverage-stats`
カバレッジ分析データ。

#### `GET /api/admin/questions/{id}`
問題詳細。

#### `DELETE /api/admin/questions/{id}`
問題削除。

#### `POST /api/admin/questions/bulk-delete`
問題一括削除。

**リクエストボディ:**
```json
{ "ids": ["uuid1", "uuid2"] }
```

#### `POST /api/admin/monitor-quality`
問題品質監視（Admin JWT または X-Scheduler-Secret ヘッダで認証）。

---

## CORS設定

| 項目 | 値 |
|------|-----|
| 許可オリジン | `FRONTEND_URL` 環境変数 |
| 許可メソッド | GET, POST, PUT, DELETE, OPTIONS |
| 許可ヘッダー | Content-Type, Authorization |
| Credentials | 有効 |
