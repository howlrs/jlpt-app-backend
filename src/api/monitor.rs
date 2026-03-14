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
/// DB内問題の重複検出・品質レポート・削除
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
                _ => {
                    return response_handler(
                        StatusCode::BAD_REQUEST,
                        "error".to_string(),
                        None,
                        Some("level は n1〜n5 を指定してください".to_string()),
                    )
                }
            }
        }
        None => vec![1, 2, 3, 4, 5],
    };

    info!(
        "品質監視開始 (execute={}, threshold={:.0}%, levels={:?})",
        execute,
        threshold * 100.0,
        target_levels
    );

    let mut level_reports = Vec::new();
    let mut all_delete_ids: Vec<String> = Vec::new();
    let mut total_questions = 0usize;
    let mut total_sub_questions = 0usize;
    let mut total_exact = 0usize;
    let mut total_similar = 0usize;

    for level_id in &target_levels {
        // DB全問題取得
        let questions: Vec<Question> = match db
            .client
            .fluent()
            .select()
            .from("questions")
            .filter(|q| q.field(firestore::path!(Question::level_id)).eq(*level_id))
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
        let mut category_groups: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
        let mut category_names: HashMap<String, String> = HashMap::new();
        let mut level_sub_count = 0usize;
        let mut answer_dist = [0usize; 4];

        for q in &questions {
            let cat_id = q.category_id.clone().unwrap_or_default();
            category_names
                .entry(cat_id.clone())
                .or_insert(q.category_name.clone());

            for sub_q in &q.sub_questions {
                level_sub_count += 1;

                if let Ok(ans) = sub_q.answer.parse::<usize>() {
                    if (1..=4).contains(&ans) {
                        answer_dist[ans - 1] += 1;
                    }
                }

                let sentence = sub_q
                    .sentence
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let correct_value = sub_q
                    .select_answer
                    .iter()
                    .find(|sa| sa.key == sub_q.answer)
                    .map(|sa| sa.value.trim().to_string())
                    .unwrap_or_default();

                if !sentence.is_empty() {
                    category_groups
                        .entry(cat_id.clone())
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

                // 完全一致チェック
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

                // 文字列長フィルタ + Levenshtein類似度チェック
                let s_len = sentence.chars().count();
                let similar = seen.iter().find(|(oi, _)| {
                    let orig_s = &items[*oi].1;
                    let o_len = orig_s.chars().count();
                    let ratio =
                        s_len.min(o_len) as f64 / s_len.max(o_len).max(1) as f64;
                    if ratio < 0.8 {
                        return false;
                    }
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

        let exact_count = duplicate_details
            .iter()
            .filter(|d| d.get("type").and_then(|v| v.as_str()) == Some("exact"))
            .count();
        let similar_count = duplicate_details.len() - exact_count;
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
        let mut categories: Vec<serde_json::Value> = category_groups
            .iter()
            .map(|(cat_id, items)| {
                let name = category_names.get(cat_id).cloned().unwrap_or_default();
                json!({ "id": cat_id, "name": name, "sub_question_count": items.len() })
            })
            .collect();
        categories.sort_by_key(|c| {
            c.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("0")
                .parse::<u32>()
                .unwrap_or(0)
        });

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
        all_delete_ids
            .into_iter()
            .filter(|id| set.insert(id.clone()))
            .collect()
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

    let total_duplicates = total_exact + total_similar;
    info!(
        "品質監視完了: questions={}, sub_questions={}, duplicates={} (exact={}, similar={}), deleted={}",
        total_questions, total_sub_questions, total_duplicates, total_exact, total_similar, deleted_count
    );

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
