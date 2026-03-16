# 学習履歴保存設計リファクタリング Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 不正解のみ保存（上限200件）+ 集計用user_statsドキュメント導入で、スケーラブルな学習履歴に改修する

**Architecture:** record_answer時に毎回user_statsドキュメント（レベル×カテゴリ別total/correct）をインクリメンタル更新し、不正解時のみuser_answersに保存（200件超で古いもの削除）。statsエンドポイントはuser_statsから単一読み取り。historyはNレベル・カテゴリ・日時・問題リンクのみ表示。

**Tech Stack:** Rust/Axum backend, Firestore, Next.js frontend

---

### Task 1: UserStats モデル追加 + record_answer 改修

**Files:**
- Modify: `src/api/answers.rs`

**Step 1: UserStatsモデルを追加**

`answers.rs` の先頭に以下を追加:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub total: u32,
    pub correct: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelStatsDoc {
    pub total: u32,
    pub correct: u32,
    pub categories: std::collections::HashMap<String, CategoryStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStatsDoc {
    pub user_id: String,
    pub total_answers: u32,
    pub total_correct: u32,
    pub levels: std::collections::HashMap<String, LevelStatsDoc>,
}
```

**Step 2: record_answer を改修**

- 正解/不正解に関わらず user_stats/{user_id} をインクリメンタル更新
- 不正解の場合のみ user_answers に保存
- 不正解保存後、件数が200超なら最古のレコードを削除

**Step 3: ビルド確認**

Run: `cargo check`

**Step 4: Commit**

```bash
git add src/api/answers.rs
git commit -m "feat: user_statsインクリメンタル更新、不正解のみuser_answers保存（上限200件）"
```

---

### Task 2: stats エンドポイント改修

**Files:**
- Modify: `src/api/answers.rs`

**Step 1: stats を user_stats から読み取りに変更**

user_stats/{user_id} から1ドキュメント読み取り → レスポンス構築。
全回答スキャンを廃止。

**Step 2: ビルド確認 + Commit**

---

### Task 3: history エンドポイント簡素化

**Files:**
- Modify: `src/api/answers.rs`

**Step 1: historyレスポンスを簡素化**

不正解のみ保存されるため is_correct フィルタ不要。
questionバッチ取得を廃止し、level_name/category_name/created_at/question_id/level_id のみ返す。

**Step 2: ビルド確認 + Commit**

---

### Task 4: フロントエンド history 表示改修

**Files:**
- Modify: `jlpt-app-frontend/src/app/mypage/history/page.tsx`

**Step 1: 表示をNレベル・カテゴリ・日時・問題リンクに変更**

- sentence表示を削除
- question_idとlevel_idを使って `/{level}/quiz?category={cat}&question_id={qid}` へのリンクを表示
- ○/× 表示を削除（全て不正解なので不要）

**Step 2: Commit**

---

### Task 5: デプロイ + 既存データ考慮

- `cargo check` でビルド確認
- issue作成、commit、push
- `bash deploy.sh` でCloud Runデプロイ
- 既存の正解データは残るが、新規保存は不正解のみになる
