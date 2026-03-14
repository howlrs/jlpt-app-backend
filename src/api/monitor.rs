use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use axum::http::HeaderMap;
use log::{info, warn};
use serde::Deserialize;
use serde_json::json;
use tokio_stream::StreamExt;

use crate::{
    api::utils::response_handler,
    common::similarity::{normalized_similarity, DEFAULT_SIMILARITY_THRESHOLD},
    models::question::Question,
};

#[derive(Deserialize, Default)]
pub struct MonitorQuery {
    pub execute: Option<bool>,
    pub level: Option<String>,
    pub threshold: Option<f64>,
}

/// 認証チェック: AdminClaims JWT または X-Scheduler-Secret ヘッダ
fn is_authorized(headers: &HeaderMap) -> bool {
    // 1. X-Scheduler-Secret ヘッダによる認証
    if let Ok(secret) = std::env::var("SCHEDULER_SECRET") {
        if let Some(header_val) = headers.get("x-scheduler-secret") {
            if let Ok(val) = header_val.to_str() {
                if val == secret {
                    return true;
                }
            }
        }
    }

    // 2. JWT Bearer トークンによる認証（AdminClaims相当）
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(val) = auth_header.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                if let Ok(token_data) = jsonwebtoken::decode::<crate::models::claim::Claims>(
                    token,
                    &jsonwebtoken::DecodingKey::from_secret(
                        std::env::var("JWT_SECRET")
                            .unwrap_or_default()
                            .as_bytes(),
                    ),
                    &jsonwebtoken::Validation::default(),
                ) {
                    if token_data.claims.role.as_deref() == Some("admin") {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// POST /api/admin/monitor-quality
/// DB内問題の重複検出・品質レポート・削除
///
/// 認証: Admin JWT または X-Scheduler-Secret ヘッダ
pub async fn monitor_quality(
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<MonitorQuery>,
    State(db): State<Arc<crate::common::database::Database>>,
) -> impl IntoResponse {
    if !is_authorized(&headers) {
        return response_handler(
            StatusCode::UNAUTHORIZED,
            "error".to_string(),
            None,
            Some("認証が必要です".to_string()),
        );
    }
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
    let mut total_malformed = 0usize;

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

        // 品質異常検出（空括弧、選択肢異常、正解キー不在等）
        let mut malformed_details = Vec::new();
        let mut malformed_ids: HashSet<String> = HashSet::new();

        for q in &questions {
            let mut issues: Vec<String> = Vec::new();
            let cat_id_num = q.category_id.as_deref().unwrap_or("0")
                .parse::<u32>().unwrap_or(0);

            for sub_q in &q.sub_questions {
                let sentence = sub_q.sentence.as_deref().unwrap_or("").trim();

                // 空括弧チェック — 漢字読み(2)・表記(3)のみ対象
                // 文脈規定(4)・文法(8)等の穴埋め問題では（　　）は正常
                let has_empty_parens = sentence.contains("（　　）")
                    || sentence.contains("（）")
                    || sentence.contains("（  ）")
                    || sentence.contains("（ ）");
                if has_empty_parens && (cat_id_num == 2 || cat_id_num == 3) {
                    issues.push("空括弧(読み/表記)".to_string());
                }

                // 選択肢数チェック
                if sub_q.select_answer.len() != 4 {
                    issues.push(format!("選択肢{}個", sub_q.select_answer.len()));
                }

                // 正解キー存在チェック
                let answer_exists = sub_q
                    .select_answer
                    .iter()
                    .any(|sa| sa.key == sub_q.answer);
                if !answer_exists {
                    issues.push("正解キー不在".to_string());
                }

                // 空の選択肢チェック
                let empty_choices = sub_q
                    .select_answer
                    .iter()
                    .filter(|sa| sa.value.trim().is_empty())
                    .count();
                if empty_choices > 0 {
                    issues.push(format!("空選択肢{}個", empty_choices));
                }

                // 空のsentenceチェック
                if sentence.is_empty() {
                    issues.push("空問題文".to_string());
                }
            }

            if !issues.is_empty() {
                let unique_issues: HashSet<String> = issues.into_iter().collect();
                malformed_details.push(json!({
                    "question_id": q.id,
                    "category_id": q.category_id,
                    "category_name": q.category_name,
                    "issues": unique_issues.into_iter().collect::<Vec<_>>(),
                }));
                malformed_ids.insert(q.id.clone());
            }
        }

        total_malformed += malformed_details.len();

        // カテゴリ内で重複検出
        let mut duplicate_details = Vec::new();
        let mut delete_ids: HashSet<String> = HashSet::new();
        // 品質異常のIDも削除対象に追加
        delete_ids.extend(malformed_ids);

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
            "malformed": malformed_details.len(),
            "answer_distribution": dist,
            "categories": categories,
            "duplicate_details": duplicate_details,
            "malformed_details": malformed_details,
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
        "品質監視完了: questions={}, sub_questions={}, duplicates={} (exact={}, similar={}), malformed={}, deleted={}",
        total_questions, total_sub_questions, total_duplicates, total_exact, total_similar, total_malformed, deleted_count
    );

    let response_data = json!({
        "summary": {
            "total_questions": total_questions,
            "total_sub_questions": total_sub_questions,
            "duplicates_found": total_duplicates,
            "duplicates_exact": total_exact,
            "duplicates_similar": total_similar,
            "malformed": total_malformed,
            "delete_targets": unique_delete.len(),
            "deleted": deleted_count,
            "executed": execute,
        },
        "levels": level_reports,
    });

    // Discord Webhook通知
    notify_discord(&response_data).await;

    response_handler(
        StatusCode::OK,
        "success".to_string(),
        Some(response_data),
        None,
    )
}

/// Discord Webhookにレポートを送信
async fn notify_discord(data: &serde_json::Value) {
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            info!("DISCORD_WEBHOOK_URL未設定 - 通知スキップ");
            return;
        }
    };

    let summary = &data["summary"];
    let total_q = summary["total_questions"].as_u64().unwrap_or(0);
    let total_sub = summary["total_sub_questions"].as_u64().unwrap_or(0);
    let dups = summary["duplicates_found"].as_u64().unwrap_or(0);
    let exact = summary["duplicates_exact"].as_u64().unwrap_or(0);
    let similar = summary["duplicates_similar"].as_u64().unwrap_or(0);
    let malformed = summary["malformed"].as_u64().unwrap_or(0);
    let deleted = summary["deleted"].as_u64().unwrap_or(0);
    let executed = summary["executed"].as_bool().unwrap_or(false);

    // レベル別サマリー
    let mut level_lines = Vec::new();
    if let Some(levels) = data["levels"].as_array() {
        for lv in levels {
            let name = lv["level"].as_str().unwrap_or("?");
            let q = lv["questions"].as_u64().unwrap_or(0);
            let sub = lv["sub_questions"].as_u64().unwrap_or(0);
            let dup = lv["duplicates"].as_u64().unwrap_or(0);
            let mal = lv["malformed"].as_u64().unwrap_or(0);
            let dist = &lv["answer_distribution"];
            let d1 = dist["1"].as_str().unwrap_or("-");
            let d2 = dist["2"].as_str().unwrap_or("-");
            let d3 = dist["3"].as_str().unwrap_or("-");
            let d4 = dist["4"].as_str().unwrap_or("-");
            level_lines.push(format!(
                "**{}** : {}問 (sub:{}) | 重複:{} 不良:{} | 正解: {}/{}/{}/{}",
                name, q, sub, dup, mal, d1, d2, d3, d4
            ));
        }
    }

    let total_issues = dups + malformed;
    let status_emoji = if total_issues == 0 { "✅" } else if deleted > 0 { "🔧" } else { "⚠️" };
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC");

    let embed = json!({
        "embeds": [{
            "title": format!("{} JLPT品質監視レポート", status_emoji),
            "color": if dups == 0 { 3066993 } else { 15844367 },
            "fields": [
                {
                    "name": "総問題数",
                    "value": format!("{} 問 ({} sub_questions)", total_q, total_sub),
                    "inline": true
                },
                {
                    "name": "重複検出",
                    "value": format!("{}件 (完全一致:{}, 類似:{})", dups, exact, similar),
                    "inline": true
                },
                {
                    "name": "品質異常",
                    "value": format!("{}件 (空括弧・選択肢異常等)", malformed),
                    "inline": true
                },
                {
                    "name": "削除",
                    "value": if executed {
                        format!("{}件 削除済み", deleted)
                    } else {
                        "未実行 (DRY RUN)".to_string()
                    },
                    "inline": true
                },
                {
                    "name": "レベル別",
                    "value": level_lines.join("\n"),
                    "inline": false
                }
            ],
            "footer": {
                "text": format!("実行時刻: {}", now)
            }
        }]
    });

    let client = reqwest::Client::new();
    match client.post(&webhook_url).json(&embed).send().await {
        Ok(res) => info!("Discord通知送信: status={}", res.status()),
        Err(e) => warn!("Discord通知失敗: {}", e),
    }
}
