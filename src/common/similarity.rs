/// 類似度の閾値（0.0〜1.0）
pub const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.85;

/// 2つの文字列の正規化類似度を計算（0.0〜1.0、1.0が完全一致）
/// Levenshtein距離ベース
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

    for j in 0..=n {
        prev[j] = j;
    }

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_strings() {
        assert_eq!(normalized_similarity("abc", "abc"), 1.0);
    }

    #[test]
    fn test_empty_strings() {
        assert_eq!(normalized_similarity("", ""), 1.0);
    }

    #[test]
    fn test_completely_different() {
        let sim = normalized_similarity("abc", "xyz");
        assert!(sim < 0.5);
    }

    #[test]
    fn test_similar_japanese() {
        let a = "彼は忙しいのに、手伝ってくれた";
        let b = "彼は忙しいのに、手伝ってくれました";
        let sim = normalized_similarity(a, b);
        assert!(sim >= 0.85, "similarity={:.2}", sim);
    }

    #[test]
    fn test_threshold() {
        assert!(DEFAULT_SIMILARITY_THRESHOLD > 0.0);
        assert!(DEFAULT_SIMILARITY_THRESHOLD <= 1.0);
    }
}
