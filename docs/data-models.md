# jlpt-app-backend データモデル

## Firestore コレクション

### `questions` コレクション

JLPT問題データ。

```rust
struct Question {
    id: String,                    // ドキュメントID（UUID）
    level_id: Option<u32>,         // JLPTレベル (1=N1, 2=N2, ..., 5=N5)
    level_name: Option<String>,    // レベル表示名 ("N1"〜"N5")
    category_id: Option<u32>,      // カテゴリID
    category_name: Option<String>, // カテゴリ名 ("文法", "語彙" 等)
    chapter: Option<String>,       // 章・セクション
    sentence: Option<String>,      // 問題文（大問）
    prerequisites: Option<String>, // 前提条件・文脈
    sub_questions: Vec<SubQuestion>, // 小問リスト
}

struct SubQuestion {
    id: u32,                       // 小問ID（連番）
    hint_id: u32,                  // ヒントID
    answer_id: u32,                // 回答ID
    sentence: Option<String>,      // 小問文
    prerequisites: Option<String>, // 前提条件
    select_answer: Vec<HashMap<String, String>>, // 選択肢 [{key, value}]
    answer: String,                // 正解 ("1"〜"4")
}
```

**備考:**
- `category_id` はString/Numberの混在に対応するカスタムデシリアライザを実装
- Firestoreの複合インデックスで `level_id` + `category_id` の絞り込みに対応

---

### `levels` コレクション

JLPTレベルマスタ。

```rust
struct Value {
    id: u32,       // レベルID (1-5)
    name: String,  // 表示名 ("N1"〜"N5")
}
```

---

### `categories` コレクション

カテゴリマスタ。

```rust
struct CatValue {
    level_id: u32,        // 所属レベルID
    id: u32,              // カテゴリID
    name: String,         // カテゴリ名
    reten: Option<u32>,   // 問題数
}
```

---

### `users` コレクション

ユーザーデータ。ドキュメントキーはメールアドレス。

```rust
struct User {
    id: String,                       // ドキュメントID
    user_id: String,                  // ユーザーID（UUID v7）
    email: String,                    // メールアドレス（キー）
    password: String,                 // パスワード（Argon2ハッシュ）
    ip: Option<String>,               // IPアドレス
    language: Option<String>,         // 言語設定
    country: Option<String>,          // 国情報
    created_at: Option<DateTime<Utc>>, // 作成日時
}
```

---

### `votes` コレクション

問題評価データ。

```rust
struct Vote {
    id: String,                 // UUID v4
    vote: String,               // "good" または "bad"
    where_to: Option<String>,   // コレクション種別 ("questions")
    parent_id: String,          // 問題ID
    child_id: String,           // 小問ID
    created_at: i64,            // Unixタイムスタンプ
}
```

## ER図（概念）

```
levels 1───* categories
               │
questions ─────┘  (level_id + category_id で関連)
    │
    └── sub_questions (埋め込み配列)

users (独立)

votes ───── questions (parent_id で参照)
```
