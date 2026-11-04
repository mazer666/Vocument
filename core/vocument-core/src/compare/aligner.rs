//! Global sequence page alignment using Needleman-Wunsch dynamic programming (`COMPARE-002`).
//!
//! Aligns pages across document versions even when pages have been inserted, deleted, or shifted.

use std::collections::HashSet;

use super::model::PageAlignment;

/// Aligns pages between Document A and Document B based on textual content similarity.
#[allow(clippy::needless_range_loop)]
pub fn align_pages(pages_text_a: &[String], pages_text_b: &[String]) -> Vec<PageAlignment> {
    let m = pages_text_a.len();
    let n = pages_text_b.len();

    if m == 0 && n == 0 {
        return Vec::new();
    }

    if m == 0 {
        return (0..n)
            .map(|j| PageAlignment {
                page_a: None,
                page_b: Some(j as u32),
                similarity_score: 0.0,
            })
            .collect();
    }

    if n == 0 {
        return (0..m)
            .map(|i| PageAlignment {
                page_a: Some(i as u32),
                page_b: None,
                similarity_score: 0.0,
            })
            .collect();
    }

    // Precompute pairwise page similarities
    let mut sim = vec![vec![0.0f32; n]; m];
    for i in 0..m {
        for j in 0..n {
            sim[i][j] = compute_page_similarity(&pages_text_a[i], &pages_text_b[j]);
        }
    }

    // Needleman-Wunsch DP matrix
    let gap_penalty = -0.4f32;
    let mut dp = vec![vec![0.0f32; n + 1]; m + 1];

    for i in 1..=m {
        dp[i][0] = i as f32 * gap_penalty;
    }
    for j in 1..=n {
        dp[0][j] = j as f32 * gap_penalty;
    }

    for i in 1..=m {
        for j in 1..=n {
            // Match score: high similarity gives positive reward, low similarity gives penalty
            let match_score = sim[i - 1][j - 1] * 2.0 - 0.5;
            let score_diag = dp[i - 1][j - 1] + match_score;
            let score_up = dp[i - 1][j] + gap_penalty;
            let score_left = dp[i][j - 1] + gap_penalty;

            dp[i][j] = score_diag.max(score_up).max(score_left);
        }
    }

    // Backtrack to extract page alignments
    let mut alignments = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 {
            let match_score = sim[i - 1][j - 1] * 2.0 - 0.5;
            let score_diag = dp[i - 1][j - 1] + match_score;
            if (dp[i][j] - score_diag).abs() < 1e-4 {
                alignments.push(PageAlignment {
                    page_a: Some((i - 1) as u32),
                    page_b: Some((j - 1) as u32),
                    similarity_score: sim[i - 1][j - 1],
                });
                i -= 1;
                j -= 1;
                continue;
            }
        }

        if i > 0 && (j == 0 || (dp[i][j] - (dp[i - 1][j] + gap_penalty)).abs() < 1e-4) {
            alignments.push(PageAlignment {
                page_a: Some((i - 1) as u32),
                page_b: None,
                similarity_score: 0.0,
            });
            i -= 1;
        } else if j > 0 {
            alignments.push(PageAlignment {
                page_a: None,
                page_b: Some((j - 1) as u32),
                similarity_score: 0.0,
            });
            j -= 1;
        }
    }

    alignments.reverse();
    alignments
}

/// Computes Jaccard similarity between word token sets of two pages (0.0 .. 1.0).
fn compute_page_similarity(text_a: &str, text_b: &str) -> f32 {
    let words_a: HashSet<&str> = text_a.split_whitespace().collect();
    let words_b: HashSet<&str> = text_b.split_whitespace().collect();

    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }

    let intersection_count = words_a.intersection(&words_b).count();
    let union_count = words_a.union(&words_b).count();

    if union_count == 0 {
        1.0
    } else {
        intersection_count as f32 / union_count as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_identical_pages() {
        let pages_a = vec![
            "Page 1 terms and conditions".to_string(),
            "Page 2 obligations".to_string(),
            "Page 3 signatures".to_string(),
        ];
        let pages_b = pages_a.clone();

        let alignments = align_pages(&pages_a, &pages_b);
        assert_eq!(alignments.len(), 3);
        assert_eq!(alignments[0].page_a, Some(0));
        assert_eq!(alignments[0].page_b, Some(0));
        assert_eq!(alignments[1].page_a, Some(1));
        assert_eq!(alignments[1].page_b, Some(1));
        assert_eq!(alignments[2].page_a, Some(2));
        assert_eq!(alignments[2].page_b, Some(2));
    }

    #[test]
    fn test_align_with_inserted_page() {
        let pages_a = vec!["Page 1 terms".to_string(), "Page 2 signatures".to_string()];
        let pages_b = vec![
            "Page 1 terms".to_string(),
            "Page NEW cover and appendix".to_string(),
            "Page 2 signatures".to_string(),
        ];

        let alignments = align_pages(&pages_a, &pages_b);
        assert_eq!(alignments.len(), 3);
        assert_eq!(alignments[0].page_a, Some(0));
        assert_eq!(alignments[0].page_b, Some(0));

        assert_eq!(alignments[1].page_a, None);
        assert_eq!(alignments[1].page_b, Some(1));

        assert_eq!(alignments[2].page_a, Some(1));
        assert_eq!(alignments[2].page_b, Some(2));
    }
}
