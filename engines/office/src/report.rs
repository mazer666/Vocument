//! Transparent reporting of unsupported properties and conversion losses (`OFFICE-004`).
//!
//! Every adapter writes into one [`ConversionReport`]. Nothing is dropped, degraded or blocked
//! without an entry, and the report travels to the user interface so the person editing the
//! document sees what a conversion costs.

use serde::{Deserialize, Serialize};

/// How serious a reported item is, from harmless to content lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Informational: nothing was lost.
    Info,
    /// Kept in the document model and written back on export, but not rendered or editable.
    PreservedUnrendered,
    /// Kept in a simplified form (for example a field result without its field code).
    Approximated,
    /// Content or formatting that was **not** carried over.
    Loss,
    /// Content neutralised for safety (macros, external resources, hostile structure).
    Security,
}

/// One kind of finding with the number of times it occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportEntry {
    pub severity: Severity,
    /// Stable machine-readable identifier such as `docx.drawing.chart`.
    pub code: String,
    /// Plain-language description for the user.
    pub message: String,
    /// How often it occurred in the document.
    pub occurrences: u32,
    /// Where the first occurrence was found, for example `word/document.xml, paragraph 12`.
    pub first_location: Option<String>,
}

/// The findings of one import or export.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversionReport {
    pub entries: Vec<ReportEntry>,
}

impl ConversionReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one occurrence of a finding. Equal `(severity, code)` pairs are merged.
    pub fn note(&mut self, severity: Severity, code: &str, message: &str, location: Option<&str>) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.severity == severity && entry.code == code)
        {
            entry.occurrences = entry.occurrences.saturating_add(1);
            return;
        }
        self.entries.push(ReportEntry {
            severity,
            code: code.to_string(),
            message: message.to_string(),
            occurrences: 1,
            first_location: location.map(str::to_string),
        });
    }

    /// Records a finding for content that was not carried over.
    pub fn loss(&mut self, code: &str, message: &str, location: Option<&str>) {
        self.note(Severity::Loss, code, message, location);
    }

    /// Number of occurrences at or above a severity.
    pub fn count_at_least(&self, severity: Severity) -> u32 {
        self.entries
            .iter()
            .filter(|entry| entry.severity >= severity)
            .map(|entry| entry.occurrences)
            .sum()
    }

    /// True when any content or formatting was lost.
    pub fn has_losses(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.severity == Severity::Loss)
    }

    /// Entries of exactly one severity.
    pub fn entries_with(&self, severity: Severity) -> impl Iterator<Item = &ReportEntry> {
        self.entries
            .iter()
            .filter(move |entry| entry.severity == severity)
    }

    /// Absorbs another report.
    pub fn merge(&mut self, other: ConversionReport) {
        for entry in other.entries {
            if let Some(existing) = self
                .entries
                .iter_mut()
                .find(|e| e.severity == entry.severity && e.code == entry.code)
            {
                existing.occurrences = existing.occurrences.saturating_add(entry.occurrences);
            } else {
                self.entries.push(entry);
            }
        }
    }

    /// Multi-line plain-language summary for dialogs and logs.
    pub fn summary_text(&self) -> String {
        if self.entries.is_empty() {
            return "No unsupported properties were found.".to_string();
        }
        let mut sorted: Vec<&ReportEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.code.cmp(&b.code)));
        let mut text = String::new();
        for entry in sorted {
            let label = match entry.severity {
                Severity::Info => "Info",
                Severity::PreservedUnrendered => "Kept, not shown",
                Severity::Approximated => "Simplified",
                Severity::Loss => "LOST",
                Severity::Security => "Blocked",
            };
            text.push_str(&format!(
                "[{label}] {} ({}×, {})",
                entry.message, entry.occurrences, entry.code
            ));
            if let Some(location) = &entry.first_location {
                text.push_str(&format!(" — first at {location}"));
            }
            text.push('\n');
        }
        text
    }

    /// JSON form for the native user interfaces.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{\"entries\":[]}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_equal_findings_and_counts_by_severity() {
        let mut report = ConversionReport::new();
        report.loss("docx.chart", "Charts are not supported", Some("p 3"));
        report.loss("docx.chart", "Charts are not supported", Some("p 9"));
        report.note(Severity::Security, "docx.macro", "Macros removed", None);
        report.note(Severity::Info, "docx.note", "fine", None);
        assert_eq!(report.entries.len(), 3);
        assert!(report.has_losses());
        assert_eq!(report.count_at_least(Severity::Loss), 3);
        assert_eq!(report.entries[0].first_location.as_deref(), Some("p 3"));
        assert!(report.summary_text().contains("LOST"));
        assert!(report.to_json().contains("docx.chart"));
    }

    #[test]
    fn an_empty_report_says_so() {
        assert!(
            ConversionReport::new()
                .summary_text()
                .contains("No unsupported")
        );
    }
}
