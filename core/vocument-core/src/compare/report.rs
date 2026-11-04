//! Exportable report formatters (JSON, Markdown, standalone HTML) for Document Comparison (`COMPARE-002`).

use std::fmt::Write as FmtWrite;

use super::model::{ChangeItem, ComparisonReport, DiffOperation};

impl ComparisonReport {
    /// Serializes comparison report to indented JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Formats comparison results into a Markdown report.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        let _ = writeln!(md, "# Vocument Document Comparison Report\n");
        let _ = writeln!(md, "- **Document A:** {}", self.doc_a_title);
        let _ = writeln!(md, "- **Document B:** {}", self.doc_b_title);
        let _ = writeln!(
            md,
            "- **Pages:** {} vs {}\n",
            self.page_count_a, self.page_count_b
        );

        let _ = writeln!(md, "## Summary\n");
        let _ = writeln!(md, "| Metric | Value |");
        let _ = writeln!(md, "| --- | --- |");
        let _ = writeln!(md, "| Total Changes | {} |", self.summary.total_changes);
        let _ = writeln!(md, "| Insertions | {} |", self.summary.insertions_count);
        let _ = writeln!(md, "| Deletions | {} |", self.summary.deletions_count);
        let _ = writeln!(md, "| Replacements | {} |", self.summary.replacements_count);
        let _ = writeln!(md, "| Moved Passages | {} |", self.summary.moves_count);
        let _ = writeln!(
            md,
            "| Content Similarity | {:.1}% |",
            self.summary.similarity_ratio * 100.0
        );
        let _ = writeln!(
            md,
            "| Identical Pages | {} |",
            self.summary.identical_pages_count
        );
        let _ = writeln!(
            md,
            "| Modified Pages | {} |",
            self.summary.modified_pages_count
        );
        let _ = writeln!(
            md,
            "| Inserted Pages | {} |",
            self.summary.inserted_pages_count
        );
        let _ = writeln!(
            md,
            "| Deleted Pages | {} |\n",
            self.summary.deleted_pages_count
        );

        if !self.page_alignments.is_empty() {
            let _ = writeln!(md, "## Page Alignment\n");
            let _ = writeln!(md, "| Doc A Page | Doc B Page | Similarity | Status |");
            let _ = writeln!(md, "| --- | --- | --- | --- |");
            for align in &self.page_alignments {
                let a_str = align
                    .page_a
                    .map_or("-".to_string(), |p| (p + 1).to_string());
                let b_str = align
                    .page_b
                    .map_or("-".to_string(), |p| (p + 1).to_string());
                let status = match (align.page_a, align.page_b) {
                    (Some(_), Some(_)) if align.similarity_score > 0.99 => "Identical",
                    (Some(_), Some(_)) => "Modified",
                    (None, Some(_)) => "Inserted in B",
                    (Some(_), None) => "Deleted from A",
                    _ => "-",
                };
                let _ = writeln!(
                    md,
                    "| {} | {} | {:.1}% | {} |",
                    a_str,
                    b_str,
                    align.similarity_score * 100.0,
                    status
                );
            }
            let _ = writeln!(md);
        }

        if !self.changes.is_empty() {
            let _ = writeln!(md, "## Detailed Changes\n");
            for change in &self.changes {
                format_change_markdown(&mut md, change);
            }
        }

        md
    }

    /// Generates a standalone, beautiful HTML comparison report with inline CSS.
    pub fn to_html(&self) -> String {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html lang=\"de\">\n<head>\n<meta charset=\"utf-8\">\n");
        html.push_str("<title>Vocument Document Comparison Report</title>\n");
        html.push_str("<style>\n");
        html.push_str("body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; margin: 30px; background: #f8f9fa; color: #212529; line-height: 1.5; }\n");
        html.push_str(".card { background: #ffffff; border-radius: 8px; padding: 24px; margin-bottom: 24px; box-shadow: 0 2px 4px rgba(0,0,0,0.08); }\n");
        html.push_str("h1, h2, h3 { color: #1a202c; }\n");
        html.push_str(".badge { display: inline-block; padding: 4px 10px; border-radius: 12px; font-weight: 600; font-size: 0.85rem; margin-right: 8px; }\n");
        html.push_str(".badge-ins { background: #d1e7dd; color: #0f5132; }\n");
        html.push_str(".badge-del { background: #f8d7da; color: #842029; }\n");
        html.push_str(".badge-rep { background: #fff3cd; color: #664d03; }\n");
        html.push_str(".badge-move { background: #cff4fc; color: #055160; }\n");
        html.push_str("table { width: 100%; border-collapse: collapse; margin-top: 12px; margin-bottom: 12px; }\n");
        html.push_str(
            "th, td { text-align: left; padding: 10px 14px; border-bottom: 1px solid #dee2e6; }\n",
        );
        html.push_str("th { background: #f1f3f5; font-weight: 600; }\n");
        html.push_str(".diff-box { border-left: 4px solid #6c757d; padding: 12px 16px; margin-bottom: 16px; background: #ffffff; border-radius: 4px; }\n");
        html.push_str(".diff-box.insert { border-left-color: #198754; }\n");
        html.push_str(".diff-box.delete { border-left-color: #dc3545; }\n");
        html.push_str(".diff-box.replace { border-left-color: #ffc107; }\n");
        html.push_str(".diff-box.move { border-left-color: #0dcaf0; }\n");
        html.push_str("ins { background: #d4edda; text-decoration: none; padding: 2px 4px; border-radius: 2px; }\n");
        html.push_str("del { background: #f8d7da; text-decoration: line-through; padding: 2px 4px; border-radius: 2px; }\n");
        html.push_str("</style>\n</head>\n<body>\n");

        html.push_str("<div class=\"card\">\n");
        html.push_str("<h1>Document Comparison Report</h1>\n");
        let _ = writeln!(
            html,
            "<p><strong>Document A:</strong> {} ({} pages) &nbsp;|&nbsp; <strong>Document B:</strong> {} ({} pages)</p>",
            escape_html(&self.doc_a_title),
            self.page_count_a,
            escape_html(&self.doc_b_title),
            self.page_count_b
        );
        let _ = writeln!(
            html,
            "<div><span class=\"badge badge-ins\">+{} Additions</span><span class=\"badge badge-del\">-{} Deletions</span><span class=\"badge badge-rep\">~{} Replacements</span><span class=\"badge badge-move\">⇄ {} Moves</span><span class=\"badge\" style=\"background:#e2e3e5;\">Similarity: {:.1}%</span></div>",
            self.summary.insertions_count,
            self.summary.deletions_count,
            self.summary.replacements_count,
            self.summary.moves_count,
            self.summary.similarity_ratio * 100.0
        );
        html.push_str("</div>\n");

        if !self.changes.is_empty() {
            html.push_str("<div class=\"card\">\n<h2>Changes List</h2>\n");
            for change in &self.changes {
                format_change_html(&mut html, change);
            }
            html.push_str("</div>\n");
        }

        html.push_str("</body>\n</html>\n");
        html
    }
}

fn format_change_markdown(md: &mut String, change: &ChangeItem) {
    let loc = match (change.page_a, change.page_b) {
        (Some(pa), Some(pb)) => format!("Page {} -> {}", pa + 1, pb + 1),
        (Some(pa), None) => format!("Page {}", pa + 1),
        (None, Some(pb)) => format!("Page {}", pb + 1),
        (None, None) => "Document".to_string(),
    };

    let clause = change
        .clause_hint
        .as_ref()
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();

    match change.operation {
        DiffOperation::Insert => {
            let _ = writeln!(
                md,
                "- **[+] Inserted** in {}{}: `{}`",
                loc,
                clause,
                change.text_b.as_deref().unwrap_or("")
            );
        }
        DiffOperation::Delete => {
            let _ = writeln!(
                md,
                "- **[-] Deleted** from {}{}: `{}`",
                loc,
                clause,
                change.text_a.as_deref().unwrap_or("")
            );
        }
        DiffOperation::Replace => {
            let _ = writeln!(
                md,
                "- **[~] Replaced** in {}{}:\n  - Before: `{}`\n  - After:  `{}`",
                loc,
                clause,
                change.text_a.as_deref().unwrap_or(""),
                change.text_b.as_deref().unwrap_or("")
            );
        }
        DiffOperation::Move { .. } => {
            let _ = writeln!(
                md,
                "- **[⇄] Moved Passage** in {}{}: `{}`",
                loc,
                clause,
                change
                    .text_b
                    .as_deref()
                    .or(change.text_a.as_deref())
                    .unwrap_or("")
            );
        }
        DiffOperation::Unchanged => {}
    }
}

fn format_change_html(html: &mut String, change: &ChangeItem) {
    let (class, title) = match change.operation {
        DiffOperation::Insert => ("insert", "Insertion"),
        DiffOperation::Delete => ("delete", "Deletion"),
        DiffOperation::Replace => ("replace", "Replacement"),
        DiffOperation::Move { .. } => ("move", "Moved Passage"),
        DiffOperation::Unchanged => ("unchanged", "Unchanged"),
    };

    let loc = match (change.page_a, change.page_b) {
        (Some(pa), Some(pb)) => format!("Page {} &rarr; {}", pa + 1, pb + 1),
        (Some(pa), None) => format!("Page {}", pa + 1),
        (None, Some(pb)) => format!("Page {}", pb + 1),
        (None, None) => "Global".to_string(),
    };

    let clause = change
        .clause_hint
        .as_ref()
        .map(|c| format!(" &bull; <em>{}</em>", escape_html(c)))
        .unwrap_or_default();

    let _ = write!(
        html,
        "<div class=\"diff-box {}\">\n<h4>{} ({}){}</h4>\n",
        class, title, loc, clause
    );

    match change.operation {
        DiffOperation::Insert => {
            let _ = writeln!(
                html,
                "<p><ins>{}</ins></p>",
                escape_html(change.text_b.as_deref().unwrap_or(""))
            );
        }
        DiffOperation::Delete => {
            let _ = writeln!(
                html,
                "<p><del>{}</del></p>",
                escape_html(change.text_a.as_deref().unwrap_or(""))
            );
        }
        DiffOperation::Replace => {
            let _ = writeln!(
                html,
                "<p><del>{}</del> &rarr; <ins>{}</ins></p>",
                escape_html(change.text_a.as_deref().unwrap_or("")),
                escape_html(change.text_b.as_deref().unwrap_or(""))
            );
        }
        DiffOperation::Move { .. } => {
            let _ = writeln!(
                html,
                "<p><span style=\"background:#e0f7fa; padding:2px 4px;\">{}</span></p>",
                escape_html(
                    change
                        .text_b
                        .as_deref()
                        .or(change.text_a.as_deref())
                        .unwrap_or("")
                )
            );
        }
        DiffOperation::Unchanged => {}
    }

    html.push_str("</div>\n");
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::model::ComparisonSummary;

    #[test]
    fn test_report_markdown_and_html_rendering() {
        let report = ComparisonReport {
            doc_a_title: "Contract_v1.pdf".to_string(),
            doc_b_title: "Contract_v2.pdf".to_string(),
            page_count_a: 10,
            page_count_b: 11,
            summary: ComparisonSummary {
                total_changes: 3,
                insertions_count: 1,
                deletions_count: 1,
                replacements_count: 1,
                moves_count: 0,
                similarity_ratio: 0.95,
                identical_pages_count: 8,
                modified_pages_count: 2,
                inserted_pages_count: 1,
                deleted_pages_count: 0,
            },
            page_alignments: Vec::new(),
            page_diffs: Vec::new(),
            changes: vec![ChangeItem {
                id: "c1".to_string(),
                operation: DiffOperation::Replace,
                page_a: Some(0),
                page_b: Some(0),
                text_a: Some("1 year".to_string()),
                text_b: Some("3 years".to_string()),
                rect_a: None,
                rect_b: None,
                clause_hint: Some("§ 2 Term".to_string()),
            }],
        };

        let md = report.to_markdown();
        assert!(md.contains("Contract_v1.pdf"));
        assert!(md.contains("1 year"));
        assert!(md.contains("3 years"));
        assert!(md.contains("§ 2 Term"));

        let html = report.to_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<ins>3 years</ins>"));
        assert!(html.contains("<del>1 year</del>"));

        let json = report.to_json().expect("json");
        assert!(json.contains("\"total_changes\": 3"));
    }
}
