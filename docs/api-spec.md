# jlpt-app-backend API仕様

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
    "levels": [
      { "id": 1, "name": "N1" }
    ],
    "categories": [
      { "level_id": 1, "id": 1, "name": "語彙", "reten": 50 }
    ]
  }
}
```

カテゴリは漢字文字数でソートされる。

**エラー:** `404 Not Found`

---

#### `GET /api/level/{level_id}/categories/{category_id}/questions`

指定レベル・カテゴリの問題を取得。

**パスパラメータ:**

| パラメータ | 型 | 説明 |
|-----------|-----|------|
| `level_id` | u32 | JLPTレベル (1-5) |
| `category_id` | u32 | カテゴリID |

**クエリパラメータ:**

| パラメータ | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| `limit` | u32 | No | 取得件数上限 |

**レスポンス:** `200 OK`
```json
{
  "message": "success",
  "data": [
    {
      "id": "uuid",
      "level_id": 3,
      "level_name": "N3",
      "category_id": 1,
      "category_name": "文法",
      "chapter": "",
      "sentence": "問題文",
      "prerequisites": "",
      "sub_questions": [
        {
          "id": 1,
          "hint_id": 0,
          "answer_id": 0,
          "sentence": "小問文",
          "select_answer": [
            { "key": "1", "value": "選択肢A" },
            { "key": "2", "value": "選択肢B" },
            { "key": "3", "value": "選択肢C" },
            { "key": "4", "value": "選択肢D" }
          ],
          "answer": "1"
        }
      ]
    }
  ]
}
```

`limit` 指定時は問題順序がランダム化される。

**エラー:** `404 Not Found`

---

#### `GET /api/evaluate/{vote}`

問題に対する評価を記録。

**パスパラメータ:**

| パラメータ | 型 | 説明 |
|-----------|-----|------|
| `vote` | String | `"good"` または `"bad"` |

**クエリパラメータ:**

| パラメータ | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| `parent_id` | String | No | 問題ID |
| `child_id` | String | No | 小問ID |

**レスポンス:** `200 OK`

**エラー:** `500 Internal Server Error`

---

### 認証API

#### `POST /api/signup`

ユーザー登録。

**リクエストボディ:**
```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

**レスポンス:** `200 OK`

**エラー:** `400 Bad Request` / `500 Internal Server Error`

---

#### `POST /api/signin`

ユーザーログイン。

**リクエストボディ:**
```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

**レスポンス:** `200 OK`
```json
{
  "message": "success",
  "data": {
    "token": "eyJhbGciOi...",
    "user": { "email": "user@example.com", ... }
  }
}
```

JWTトークンの有効期限: 72時間。

**エラー:** `400 Bad Request` / `401 Unauthorized` / `500 Internal Server Error`

---

### プライベートAPI（JWT認証必須）

#### `GET /api/private/health`

認証済みヘルスチェック。

**ヘッダー:** `Authorization: Bearer <token>`

**レスポンス:** `200 OK`
```json
{
  "message": "success",
  "data": { "health": "ok", "server_time": "2025-03-06T12:00:00Z" }
}
```

## 認証方式

- **アルゴリズム:** HS256 (HMAC-SHA256)
- **トークン形式:** JWT Bearer Token
- **有効期限:** 72時間
- **Claims:** `{ user_id, email, exp }`
- **パスワードハッシュ:** Argon2id

## CORS設定

- **許可オリジン:** `FRONTEND_URL` 環境変数で設定
- **許可メソッド:** GET, POST, PUT, DELETE, OPTIONS
- **許可ヘッダー:** Content-Type, Authorization
