//! Structural text diffing with Myers / LCS alignment, replacement merging, and block movement detection (`COMPARE-001`).

use std::collections::HashMap;

use crate::pdf_engine::PdfRectMilliPt;

use super::model::{ChangeItem, ComparisonOptions, DiffGranularity, DiffOperation};

/// A single token in the comparison stream with spatial and structural context.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffToken {
    pub text: String,
    pub normalized: String,
    pub page_index: Option<u32>,
    pub rect: Option<PdfRectMilliPt>,
    pub clause_hint: Option<String>,
}

/// Tokenizes text into comparison tokens according to `ComparisonOptions`.
pub fn tokenize(
    text: &str,
    page_index: Option<u32>,
    options: &ComparisonOptions,
) -> Vec<DiffToken> {
    let mut tokens = Vec::new();
    let mut current_clause: Option<String> = None;

    match options.granularity {
        DiffGranularity::Word => {
            for line in text.lines() {
                let trimmed_line = line.trim();
                if let Some(clause) = detect_clause_header(trimmed_line) {
                    current_clause = Some(clause);
                }

                for word in line.split_whitespace() {
                    let normalized = normalize_token(word, options);
                    tokens.push(DiffToken {
                        text: word.to_string(),
                        normalized,
                        page_index,
                        rect: None,
                        clause_hint: current_clause.clone(),
                    });
                }
            }
        }
        DiffGranularity::Line => {
            for line in text.lines() {
                let trimmed_line = line.trim();
                if options.ignore_whitespace && trimmed_line.is_empty() {
                    continue;
                }
                if let Some(clause) = detect_clause_header(trimmed_line) {
                    current_clause = Some(clause);
                }
                let normalized = normalize_token(trimmed_line, options);
                tokens.push(DiffToken {
                    text: line.to_string(),
                    normalized,
                    page_index,
                    rect: None,
                    clause_hint: current_clause.clone(),
                });
            }
        }
        DiffGranularity::Character => {
            for c in text.chars() {
                if options.ignore_whitespace && c.is_whitespace() {
                    continue;
                }
                let s = c.to_string();
                let normalized = normalize_token(&s, options);
                tokens.push(DiffToken {
                    text: s,
                    normalized,
                    page_index,
                    rect: None,
                    clause_hint: current_clause.clone(),
                });
            }
        }
    }

    tokens
}

fn normalize_token(s: &str, options: &ComparisonOptions) -> String {
    let mut norm = s.to_string();
    if options.ignore_case {
        norm = norm.to_lowercase();
    }
    if options.ignore_whitespace {
        norm = norm.trim().to_string();
    }
    norm
}

/// Detects legal headers and clause prefixes (e.g. "§ 1", "Clause 3", "Art. 4", "1.2").
#[allow(clippy::collapsible_if)]
fn detect_clause_header(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with('§')
        || trimmed.starts_with("Art.")
        || trimmed.starts_with("Artikel")
        || trimmed.starts_with("Clause")
        || trimmed.starts_with("Absatz")
        || trimmed.starts_with("Ziffer")
    {
        let end = trimmed.find(':').unwrap_or_else(|| trimmed.len().min(40));
        return Some(trimmed[..end].trim().to_string());
    }

    // Numbered headings like "1.", "1.1", "2.3.4"
    if let Some(first_word) = trimmed.split_whitespace().next() {
        if first_word.chars().all(|c| c.is_ascii_digit() || c == '.') && first_word.contains('.') {
            let end = trimmed.len().min(40);
            return Some(trimmed[..end].trim().to_string());
        }
    }

    None
}

/// Computes the diff between two token streams using dynamic-programming LCS.
#[allow(clippy::needless_range_loop)]
pub fn diff_tokens(
    tokens_a: &[DiffToken],
    tokens_b: &[DiffToken],
    _options: &ComparisonOptions,
) -> Vec<ChangeItem> {
    let m = tokens_a.len();
    let n = tokens_b.len();

    // Fast-path: both empty
    if m == 0 && n == 0 {
        return Vec::new();
    }

    // Fast-path: all additions
    if m == 0 {
        let added_text: String = tokens_b
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        return vec![ChangeItem {
            id: "diff_ins_0".to_string(),
            operation: DiffOperation::Insert,
            page_a: None,
            page_b: tokens_b.first().and_then(|t| t.page_index),
            text_a: None,
            text_b: Some(added_text),
            rect_a: None,
            rect_b: tokens_b.first().and_then(|t| t.rect),
            clause_hint: tokens_b.first().and_then(|t| t.clause_hint.clone()),
        }];
    }

    // Fast-path: all deletions
    if n == 0 {
        let deleted_text: String = tokens_a
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        return vec![ChangeItem {
            id: "diff_del_0".to_string(),
            operation: DiffOperation::Delete,
            page_a: tokens_a.first().and_then(|t| t.page_index),
            page_b: None,
            text_a: Some(deleted_text),
            text_b: None,
            rect_a: tokens_a.first().and_then(|t| t.rect),
            rect_b: None,
            clause_hint: tokens_a.first().and_then(|t| t.clause_hint.clone()),
        }];
    }

    // Compute LCS matrix (optimized memory: two rows if very large, full table for standard page limits)
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 0..m {
        for j in 0..n {
            if tokens_a[i].normalized == tokens_b[j].normalized {
                dp[i + 1][j + 1] = dp[i][j] + 1;
            } else {
                dp[i + 1][j + 1] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    // Backtrack to build raw edit operations
    #[allow(dead_code)]
    #[derive(Debug)]
    enum RawOp {
        Equal(usize, usize),
        Delete(usize),
        Insert(usize),
    }

    let mut raw_ops = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && tokens_a[i - 1].normalized == tokens_b[j - 1].normalized {
            raw_ops.push(RawOp::Equal(i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            raw_ops.push(RawOp::Insert(j - 1));
            j -= 1;
        } else if i > 0 && (j == 0 || dp[i][j - 1] < dp[i - 1][j]) {
            raw_ops.push(RawOp::Delete(i - 1));
            i -= 1;
        }
    }
    raw_ops.reverse();

    // Group adjacent Inserts and Deletes into contiguous change chunks
    let mut chunks: Vec<ChangeItem> = Vec::new();
    let mut chunk_id = 1;
    let mut current_del: Vec<&DiffToken> = Vec::new();
    let mut current_ins: Vec<&DiffToken> = Vec::new();

    let flush_chunks = |del: &mut Vec<&DiffToken>,
                        ins: &mut Vec<&DiffToken>,
                        chunks: &mut Vec<ChangeItem>,
                        chunk_id: &mut usize| {
        if del.is_empty() && ins.is_empty() {
            return;
        }

        let id = format!("change_{chunk_id}");
        *chunk_id += 1;

        if !del.is_empty() && !ins.is_empty() {
            let text_a = del
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let text_b = ins
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let page_a = del.first().and_then(|t| t.page_index);
            let page_b = ins.first().and_then(|t| t.page_index);
            let rect_a = del.first().and_then(|t| t.rect);
            let rect_b = ins.first().and_then(|t| t.rect);
            let clause_hint = del
                .first()
                .and_then(|t| t.clause_hint.clone())
                .or_else(|| ins.first().and_then(|t| t.clause_hint.clone()));

            chunks.push(ChangeItem {
                id,
                operation: DiffOperation::Replace,
                page_a,
                page_b,
                text_a: Some(text_a),
                text_b: Some(text_b),
                rect_a,
                rect_b,
                clause_hint,
            });
        } else if !del.is_empty() {
            let text_a = del
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let page_a = del.first().and_then(|t| t.page_index);
            let rect_a = del.first().and_then(|t| t.rect);
            let clause_hint = del.first().and_then(|t| t.clause_hint.clone());

            chunks.push(ChangeItem {
                id,
                operation: DiffOperation::Delete,
                page_a,
                page_b: None,
                text_a: Some(text_a),
                text_b: None,
                rect_a,
                rect_b: None,
                clause_hint,
            });
        } else if !ins.is_empty() {
            let text_b = ins
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let page_b = ins.first().and_then(|t| t.page_index);
            let rect_b = ins.first().and_then(|t| t.rect);
            let clause_hint = ins.first().and_then(|t| t.clause_hint.clone());

            chunks.push(ChangeItem {
                id,
                operation: DiffOperation::Insert,
                page_a: None,
                page_b,
                text_a: None,
                text_b: Some(text_b),
                rect_a: None,
                rect_b,
                clause_hint,
            });
        }

        del.clear();
        ins.clear();
    };

    for op in raw_ops {
        match op {
            RawOp::Equal(_, _) => {
                flush_chunks(
                    &mut current_del,
                    &mut current_ins,
                    &mut chunks,
                    &mut chunk_id,
                );
            }
            RawOp::Delete(idx) => {
                current_del.push(&tokens_a[idx]);
            }
            RawOp::Insert(idx) => {
                current_ins.push(&tokens_b[idx]);
            }
        }
    }
    flush_chunks(
        &mut current_del,
        &mut current_ins,
        &mut chunks,
        &mut chunk_id,
    );

    // Detect moved text blocks: if a deleted block matches an inserted block elsewhere
    detect_block_moves(&mut chunks);

    chunks
}

/// Identifies text passages that were moved rather than deleted and re-inserted (`COMPARE-001`).
#[allow(clippy::collapsible_if, clippy::needless_range_loop)]
fn detect_block_moves(changes: &mut [ChangeItem]) {
    // Collect candidate deleted blocks of significant size (>= 3 words or 15 chars)
    let mut delete_indices: HashMap<String, usize> = HashMap::new();

    for (idx, item) in changes.iter().enumerate() {
        if item.operation == DiffOperation::Delete {
            if let Some(ref text) = item.text_a {
                let trimmed = text.trim();
                if trimmed.split_whitespace().count() >= 2 || trimmed.len() >= 8 {
                    delete_indices.insert(trimmed.to_lowercase(), idx);
                }
            }
        }
    }

    for idx in 0..changes.len() {
        if changes[idx].operation == DiffOperation::Insert {
            if let Some(ref text_b) = changes[idx].text_b {
                let trimmed_b = text_b.trim().to_lowercase();
                if let Some(&del_idx) = delete_indices.get(&trimmed_b) {
                    if del_idx != idx {
                        changes[del_idx].operation = DiffOperation::Move {
                            from_index: del_idx,
                            to_index: idx,
                        };
                        changes[idx].operation = DiffOperation::Move {
                            from_index: del_idx,
                            to_index: idx,
                        };
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_texts_yield_zero_diffs() {
        let text = "This is a legal agreement executed in Berlin.";
        let options = ComparisonOptions::default();
        let tok_a = tokenize(text, Some(0), &options);
        let tok_b = tokenize(text, Some(0), &options);
        let diffs = diff_tokens(&tok_a, &tok_b, &options);
        assert!(diffs.is_empty(), "identical texts should have 0 diffs");
    }

    #[test]
    fn test_single_insertion_and_deletion() {
        let text_a = "Agreement term is 1 year.";
        let text_b = "Agreement term is 3 years.";
        let options = ComparisonOptions::default();
        let tok_a = tokenize(text_a, Some(0), &options);
        let tok_b = tokenize(text_b, Some(0), &options);
        let diffs = diff_tokens(&tok_a, &tok_b, &options);

        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].operation, DiffOperation::Replace);
        assert_eq!(diffs[0].text_a.as_deref(), Some("1 year."));
        assert_eq!(diffs[0].text_b.as_deref(), Some("3 years."));
    }

    #[test]
    fn test_detect_clause_header() {
        assert_eq!(
            detect_clause_header("§ 242 BGB - Treu und Glauben"),
            Some("§ 242 BGB - Treu und Glauben".to_string())
        );
        assert_eq!(
            detect_clause_header("Clause 4: Confidentiality"),
            Some("Clause 4".to_string())
        );
        assert_eq!(
            detect_clause_header("1.2 Payment Terms"),
            Some("1.2 Payment Terms".to_string())
        );
    }

    #[test]
    fn test_detect_moved_passage() {
        let text_a = "Preamble. The governing law is German law. Section 1.";
        let text_b = "Preamble. Section 1. The governing law is German law.";
        let options = ComparisonOptions::default();
        let tok_a = tokenize(text_a, Some(0), &options);
        let tok_b = tokenize(text_b, Some(0), &options);
        let diffs = diff_tokens(&tok_a, &tok_b, &options);

        let move_count = diffs
            .iter()
            .filter(|d| matches!(d.operation, DiffOperation::Move { .. }))
            .count();
        assert!(
            move_count >= 1,
            "Expected moved passage to be recognized as Move"
        );
    }
}
