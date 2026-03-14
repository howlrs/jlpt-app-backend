# 問題品質監視エンドポイント 実装計画

> **Status: COMPLETED (2026-03-14)** — 全タスク実装済み（エンドポイント・Discord通知・スケジューラ認証・構造異常検出）

**Goal:** `POST /api/admin/monitor-quality` エンドポイントを追加し、DB内問題の重複検出・削除・品質レポート返却を行う

**Architecture:** 既存の admin.rs パターンに従い、AdminClaims認証付きエンドポイントを追加。Levenshtein重複検出ロジックを `common/similarity.rs` に実装し、admin.rs のハンドラから呼び出す。全問題をレベル別にFirestoreから取得し、カテゴリ内グルーピング→文字列長フィルタ→Levenshtein比較の3段階で重複を検出。

**Tech Stack:** Rust, Axum 0.8.1, Firestore SDK, Levenshtein距離

**GitHub Issue:** howlrs/jlpt-app-backend#9

---

### Task 1: Levenshtein類似度モジュール作成

**Files:**
- Create: `src/common/similarity.rs`
- Modify: `src/common/mod.rs`

**Step 1: `src/common/similarity.rs` を作成**

```rust
/// 類似度の閾値（0.0〜1.0）
pub const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.85;

/// 2つの文字列の正規化類似度を計算（0.0〜1.0、1.0が完全一致）
pub fn normalized_similarity(a: &str, b: &str) -> f64 {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let max_len = a_chars.len().max(b_chars.len());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein_distance(&a_chars, &b_chars);
    1.0 - (dist as f64 / max_len as f64)
}

/// Levenshtein距離をDP法で計算（省メモリ版）
fn levenshtein_distance(a: &[char], b: &[char]) -> usize {
    let (m, n) = (a.len(), b.len());
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];
    for j in 0..=n { prev[j] = j; }
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}
```

**Step 2: `src/common/mod.rs` にモジュール追加**

```rust
pub mod database;
pub mod similarity;
```

**Step 3: ビルド確認**

Run: `cargo build`
Expected: コンパイル成功

**Step 4: コミット**

```bash
git add src/common/similarity.rs src/common/mod.rs
git commit -m "feat: Levenshtein類似度モジュール追加"
```

---

### Task 2: 品質監視ハンドラ実装

**Files:**
- Create: `src/api/monitor.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs`

**Step 1: `src/api/monitor.rs` を作成**

品質監視の本体ロジック。既存 admin.rs のパターン（AdminClaims, State, response_handler）に準拠。

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use log::{info, warn};
use serde::Deserialize;
use serde_json::json;
use tokio_stream::StreamExt;

use crate::{
    api::utils::response_handler,
    common::similarity::{normalized_similarity, DEFAULT_SIMILARITY_THRESHOLD},
    models::{claim::AdminClaims, question::Question},
};

#[derive(Deserialize, Default)]
pub struct MonitorQuery {
    pub execute: Option<bool>,
    pub level: Option<String>,
    pub threshold: Option<f64>,
}

/// POST /api/admin/monitor-quality
pub async fn monitor_quality(
    _admin: AdminClaims,
    axum::extract::Query(query): axum::extract::Query<MonitorQuery>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    let execute = query.execute.unwrap_or(false);
    let threshold = query.threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);
    let target_levels: Vec<u32> = match &query.level {
        Some(l) => {
            let n = l.trim_start_matches('n').trim_start_matches('N');
            match n.parse::<u32>() {
                Ok(id) if (1..=5).contains(&id) => vec![id],
                _ => return response_handler(
                    StatusCode::BAD_REQUEST,
                    "error".to_string(),
                    None,
                    Some("level は n1〜n5 を指定してください".to_string()),
                ),
            }
        }
        None => vec![1, 2, 3, 4, 5],
    };

    info!("品質監視開始 (execute={}, threshold={:.0}%, levels={:?})", execute, threshold * 100.0, target_levels);

    let mut level_reports = Vec::new();
    let mut all_delete_ids: Vec<String> = Vec::new();
    let mut total_questions = 0usize;
    let mut total_sub_questions = 0usize;
    let mut total_duplicates = 0usize;
    let mut total_exact = 0usize;
    let mut total_similar = 0usize;

    for level_id in &target_levels {
        // DB全問題取得
        let questions: Vec<Question> = match db
            .client
            .fluent()
            .select()
            .from("questions")
            .filter(|q| {
                q.field(firestore::path!(Question::level_id)).eq(*level_id)
            })
            .obj::<Question>()
            .stream_query_with_errors()
            .await
        {
            Ok(stream) => {
                let mut items = Vec::new();
                let mut stream = stream;
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(q) => items.push(q),
                        Err(e) => warn!("N{} ドキュメント読取エラー: {}", level_id, e),
                    }
                }
                items
            }
            Err(e) => {
                warn!("N{} クエリエラー: {}", level_id, e);
                continue;
            }
        };

        let level_q_count = questions.len();
        total_questions += level_q_count;

        // カテゴリ別グルーピング
        // (cat_id) -> Vec<(doc_id, sentence, correct_value)>
        let mut category_groups: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
        let mut category_names: HashMap<String, String> = HashMap::new();
        let mut level_sub_count = 0usize;
        let mut answer_dist = [0usize; 4];

        for q in &questions {
            let cat_id = q.category_id.clone().unwrap_or_default();
            category_names.entry(cat_id.clone()).or_insert(q.category_name.clone());

            for sub_q in &q.sub_questions {
                level_sub_count += 1;

                if let Ok(ans) = sub_q.answer.parse::<usize>() {
                    if ans >= 1 && ans <= 4 {
                        answer_dist[ans - 1] += 1;
                    }
                }

                let sentence = sub_q.sentence.as_deref().unwrap_or("").trim().to_string();
                let correct_value = sub_q.select_answer.iter()
                    .find(|sa| sa.key == sub_q.answer)
                    .map(|sa| sa.value.trim().to_string())
                    .unwrap_or_default();

                if !sentence.is_empty() {
                    category_groups.entry(cat_id.clone())
                        .or_default()
                        .push((q.id.clone(), sentence, correct_value));
                }
            }
        }

        total_sub_questions += level_sub_count;

        // カテゴリ内で重複検出
        let mut duplicate_details = Vec::new();
        let mut delete_ids: HashSet<String> = HashSet::new();

        for (_cat_id, items) in &category_groups {
            let mut seen: Vec<(usize, String)> = Vec::new();

            for (idx, (doc_id, sentence, correct_value)) in items.iter().enumerate() {
                let dedup_key = format!("{}||{}", sentence, correct_value);

                // 完全一致
                if let Some((orig_idx, _)) = seen.iter().find(|(_, key)| key == &dedup_key) {
                    duplicate_details.push(json!({
                        "type": "exact",
                        "similarity": "100%",
                        "question_id_a": items[*orig_idx].0,
                        "question_id_b": doc_id,
                        "sentence_a": items[*orig_idx].1,
                        "sentence_b": sentence,
                    }));
                    delete_ids.insert(doc_id.clone());
                    continue;
                }

                // 文字列長フィルタ + Levenshtein
                let s_len = sentence.chars().count();
                let similar = seen.iter().find(|(oi, _)| {
                    let orig_s = &items[*oi].1;
                    let o_len = orig_s.chars().count();
                    let ratio = s_len.min(o_len) as f64 / s_len.max(o_len).max(1) as f64;
                    if ratio < 0.8 { return false; }
                    normalized_similarity(sentence, orig_s) >= threshold
                });

                if let Some((orig_idx, _)) = similar {
                    let sim = normalized_similarity(sentence, &items[*orig_idx].1);
                    duplicate_details.push(json!({
                        "type": "similar",
                        "similarity": format!("{:.0}%", sim * 100.0),
                        "question_id_a": items[*orig_idx].0,
                        "question_id_b": doc_id,
                        "sentence_a": items[*orig_idx].1,
                        "sentence_b": sentence,
                    }));
                    delete_ids.insert(doc_id.clone());
                    continue;
                }

                seen.push((idx, dedup_key));
            }
        }

        let exact_count = duplicate_details.iter()
            .filter(|d| d.get("type").and_then(|v| v.as_str()) == Some("exact"))
            .count();
        let similar_count = duplicate_details.len() - exact_count;
        total_duplicates += duplicate_details.len();
        total_exact += exact_count;
        total_similar += similar_count;

        // 正解分布
        let ans_total: usize = answer_dist.iter().sum();
        let dist = if ans_total > 0 {
            json!({
                "1": format!("{:.0}%", answer_dist[0] as f64 / ans_total as f64 * 100.0),
                "2": format!("{:.0}%", answer_dist[1] as f64 / ans_total as f64 * 100.0),
                "3": format!("{:.0}%", answer_dist[2] as f64 / ans_total as f64 * 100.0),
                "4": format!("{:.0}%", answer_dist[3] as f64 / ans_total as f64 * 100.0),
            })
        } else {
            json!({})
        };

        // カテゴリ別集計
        let mut categories: Vec<serde_json::Value> = category_groups.iter()
            .map(|(cat_id, items)| {
                let name = category_names.get(cat_id).cloned().unwrap_or_default();
                json!({ "id": cat_id, "name": name, "sub_question_count": items.len() })
            })
            .collect();
        categories.sort_by_key(|c| c.get("id").and_then(|v| v.as_str()).unwrap_or("0").parse::<u32>().unwrap_or(0));

        all_delete_ids.extend(delete_ids);

        level_reports.push(json!({
            "level": format!("N{}", level_id),
            "questions": level_q_count,
            "sub_questions": level_sub_count,
            "duplicates": duplicate_details.len(),
            "duplicates_exact": exact_count,
            "duplicates_similar": similar_count,
            "answer_distribution": dist,
            "categories": categories,
            "duplicate_details": duplicate_details,
        }));
    }

    // 削除実行
    let mut deleted_count = 0usize;
    let unique_delete: Vec<String> = {
        let mut set = HashSet::new();
        all_delete_ids.into_iter().filter(|id| set.insert(id.clone())).collect()
    };

    if execute && !unique_delete.is_empty() {
        for qid in &unique_delete {
            match db.delete("questions", qid).await {
                Ok(_) => {
                    deleted_count += 1;
                    info!("削除: {}", qid);
                }
                Err(e) => warn!("削除失敗 {}: {}", qid, e),
            }
        }
        info!("{}件削除完了", deleted_count);
    }

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(json!({
            "summary": {
                "total_questions": total_questions,
                "total_sub_questions": total_sub_questions,
                "duplicates_found": total_duplicates,
                "duplicates_exact": total_exact,
                "duplicates_similar": total_similar,
                "delete_targets": unique_delete.len(),
                "deleted": deleted_count,
                "executed": execute,
            },
            "levels": level_reports,
        })),
        None,
    )
}
```

**Step 2: `src/api/mod.rs` にモジュール追加**

```
pub mod monitor;
```

**Step 3: `src/main.rs` にルーティング追加**

```
.route("/api/admin/monitor-quality", post(api::monitor::monitor_quality))
```

**Step 4: ビルド確認**

Run: `cargo build`
Expected: コンパイル成功

**Step 5: コミット**

```bash
git add src/api/monitor.rs src/api/mod.rs src/main.rs
git commit -m "feat: 品質監視エンドポイント POST /api/admin/monitor-quality"
```

---

### Task 3: 動作確認

**Step 1: ローカルサーバー起動**

Run: `cargo run`

**Step 2: Admin JWTを取得してエンドポイントをテスト**

```bash
# レポートのみ（DRY RUN）
curl -X POST "http://localhost:8080/api/admin/monitor-quality" \
  -H "Authorization: Bearer <admin_jwt>"

# N3のみ
curl -X POST "http://localhost:8080/api/admin/monitor-quality?level=n3" \
  -H "Authorization: Bearer <admin_jwt>"

# 削除実行
curl -X POST "http://localhost:8080/api/admin/monitor-quality?execute=true" \
  -H "Authorization: Bearer <admin_jwt>"
```

Expected: JSON品質レポートが返却される
