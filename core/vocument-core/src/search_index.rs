//! Progressive local SQLite FTS5 search index and classic exact search (`SEARCH-001`, `SEARCH-002`, `SEARCH-009`, `ARCHITECTURE.md` §17, `ROADMAP.md` M0.3).
//!
//! Architecture & Gate rules (`ARCHITECTURE.md` §17, `ROADMAP.md` M0.3 Gate):
//! - Initial exact/classic backend: **SQLite FTS5**.
//! - Index progressively (`index_next_batch`) so large-document indexing never blocks the UI (`SEARCH-009`).
//! - Visible and nearby pages are indexed first.
//! - **M0.3 Gate:** Search begins returning useful results before a document is fully indexed.
//! - Every search hit maps to `page_index`, `physical_page_number`, `logical_page_label`, `snippet`,
//!   `match_count`, and `highlight_rects` (`Vec<PdfRectMilliPt>`).

use crate::navigation::PageLabelCatalog;
use crate::pdf_engine::{PdfPageStructuredText, PdfRectMilliPt};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Progress snapshot of the progressive SQLite FTS5 index for a `DocumentSession` (`SEARCH-009`, `ROADMAP.md` M0.3 Gate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchIndexProgress {
    pub indexed_pages: u32,
    pub total_pages: u32,
    pub is_complete: bool,
}

/// A single page-level match returned by classic exact FTS5 search (`SEARCH-002`, `ARCHITECTURE.md` §17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub page_index: u32,
    pub physical_page_number: u32,
    pub logical_page_label: String,
    pub snippet: String,
    pub match_count: u32,
    pub highlight_rects: Vec<PdfRectMilliPt>,
}

/// Complete response from an exact search query, including current progressive indexing status (`ROADMAP.md` M0.3 Gate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub progress: SearchIndexProgress,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SearchIndexError {
    #[error("SQLite FTS5 error: {details}")]
    SqliteFailure { details: String },
}

/// Disposable, progressive SQLite FTS5 search index owned by a `DocumentSession` (`ARCHITECTURE.md` §§10, 17).
pub struct SessionSearchIndex {
    total_pages: u32,
    conn: Connection,
    indexed_pages: BTreeSet<u32>,
    structured_pages: BTreeMap<u32, PdfPageStructuredText>,
}

impl std::fmt::Debug for SessionSearchIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSearchIndex")
            .field("total_pages", &self.total_pages)
            .field("indexed_pages_count", &self.indexed_pages.len())
            .finish()
    }
}

impl SessionSearchIndex {
    /// Creates a new progressive in-memory SQLite FTS5 index for a document with `total_pages`.
    pub fn new(total_pages: u32) -> Result<Self, SearchIndexError> {
        let conn = Connection::open_in_memory().map_err(|err| SearchIndexError::SqliteFailure {
            details: err.to_string(),
        })?;

        conn.execute_batch(
            "CREATE VIRTUAL TABLE page_fts USING fts5(
                page_index UNINDEXED,
                body_text,
                tokenize = 'unicode61 remove_diacritics 2'
            );",
        )
        .map_err(|err| SearchIndexError::SqliteFailure {
            details: err.to_string(),
        })?;

        Ok(Self {
            total_pages,
            conn,
            indexed_pages: BTreeSet::new(),
            structured_pages: BTreeMap::new(),
        })
    }

    /// Returns the current progressive indexing status (`indexed_pages`, `total_pages`, `is_complete`).
    pub fn progress(&self) -> SearchIndexProgress {
        let indexed = self.indexed_pages.len() as u32;
        SearchIndexProgress {
            indexed_pages: indexed,
            total_pages: self.total_pages,
            is_complete: indexed >= self.total_pages,
        }
    }

    /// Returns whether `page_index` has already been indexed in SQLite FTS5.
    pub fn is_page_indexed(&self, page_index: u32) -> bool {
        self.indexed_pages.contains(&page_index)
    }

    /// Returns a cached `PdfPageStructuredText` if `page_index` has already been extracted.
    pub fn cached_structured_page(&self, page_index: u32) -> Option<&PdfPageStructuredText> {
        self.structured_pages.get(&page_index)
    }

    /// Selects up to `max_pages` unindexed page indices prioritized around `preferred_start_page`
    /// (visible page first, then alternating forward/backward neighbours, then remaining pages) (`ARCHITECTURE.md` §17).
    pub fn next_unindexed_pages(&self, preferred_start_page: u32, max_pages: u32) -> Vec<u32> {
        if max_pages == 0 || self.total_pages == 0 {
            return Vec::new();
        }
        let start = preferred_start_page.min(self.total_pages.saturating_sub(1));
        let mut candidates = Vec::new();

        if !self.indexed_pages.contains(&start) {
            candidates.push(start);
        }

        let mut delta = 1u32;
        while candidates.len() < max_pages as usize && delta < self.total_pages {
            if let Some(next_p) = start.checked_add(delta)
                && next_p < self.total_pages
                && !self.indexed_pages.contains(&next_p)
            {
                candidates.push(next_p);
                if candidates.len() >= max_pages as usize {
                    break;
                }
            }
            if let Some(prev_p) = start.checked_sub(delta)
                && !self.indexed_pages.contains(&prev_p)
            {
                candidates.push(prev_p);
            }
            delta += 1;
        }

        candidates
    }

    /// Inserts or replaces a single extracted page into the SQLite FTS5 virtual table (`page_fts`).
    pub fn index_structured_page(
        &mut self,
        structured: PdfPageStructuredText,
    ) -> Result<SearchIndexProgress, SearchIndexError> {
        let page_index = structured.page_index;
        if page_index >= self.total_pages {
            return Ok(self.progress());
        }

        let joined_body: String = structured
            .lines
            .iter()
            .map(|l| l.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        if self.indexed_pages.contains(&page_index) {
            self.conn
                .execute(
                    "DELETE FROM page_fts WHERE rowid = ?1",
                    params![i64::from(page_index) + 1],
                )
                .map_err(|err| SearchIndexError::SqliteFailure {
                    details: err.to_string(),
                })?;
        }

        self.conn
            .execute(
                "INSERT INTO page_fts(rowid, page_index, body_text) VALUES (?1, ?2, ?3)",
                params![
                    i64::from(page_index) + 1,
                    i64::from(page_index),
                    joined_body
                ],
            )
            .map_err(|err| SearchIndexError::SqliteFailure {
                details: err.to_string(),
            })?;

        self.indexed_pages.insert(page_index);
        self.structured_pages.insert(page_index, structured);
        Ok(self.progress())
    }

    /// Executes a classic exact search against the SQLite FTS5 index (`SEARCH-002`).
    /// Works immediately even while progressive indexing is still incomplete (`ROADMAP.md` M0.3 Gate).
    pub fn search_exact(
        &self,
        query: &str,
        label_catalog: &PageLabelCatalog,
    ) -> Result<SearchResponse, SearchIndexError> {
        let trimmed = query.trim();
        let progress = self.progress();
        if trimmed.is_empty() {
            return Ok(SearchResponse {
                query: String::new(),
                hits: Vec::new(),
                progress,
            });
        }

        let fts_phrase = build_fts5_phrase_query(trimmed);
        let mut stmt = self
            .conn
            .prepare(
                "SELECT page_index, body_text
                 FROM page_fts
                 WHERE page_fts MATCH ?1
                 ORDER BY rowid ASC",
            )
            .map_err(|err| SearchIndexError::SqliteFailure {
                details: err.to_string(),
            })?;

        let rows = stmt
            .query_map(params![fts_phrase], |row| {
                let page_idx_i64: i64 = row.get(0)?;
                let body_text: String = row.get(1)?;
                Ok((page_idx_i64 as u32, body_text))
            })
            .map_err(|err| SearchIndexError::SqliteFailure {
                details: err.to_string(),
            })?;

        let needle_lower = trimmed.to_lowercase();
        let tokens_lower: Vec<String> = trimmed
            .split_whitespace()
            .map(|t| t.to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();

        let mut hits = Vec::new();
        for row_res in rows {
            let (page_index, body_text) =
                row_res.map_err(|err| SearchIndexError::SqliteFailure {
                    details: err.to_string(),
                })?;

            let mut highlight_rects = Vec::new();
            let mut match_count = 0u32;
            let mut first_matching_line: Option<String> = None;

            if let Some(structured) = self.structured_pages.get(&page_index) {
                for line in &structured.lines {
                    let line_lower = line.text.to_lowercase();
                    let line_matches = line_lower.contains(&needle_lower)
                        || tokens_lower.iter().any(|tok| line_lower.contains(tok));
                    if line_matches {
                        highlight_rects.push(line.bbox);
                        let occurrences = line_lower.matches(&needle_lower).count().max(1) as u32;
                        match_count = match_count.saturating_add(occurrences);
                        if first_matching_line.is_none() {
                            first_matching_line = Some(line.text.trim().to_owned());
                        }
                    }
                }
            }

            if match_count == 0 {
                match_count = 1;
            }
            let snippet = first_matching_line
                .unwrap_or_else(|| body_text.lines().next().unwrap_or("").trim().to_owned());

            hits.push(SearchHit {
                page_index,
                physical_page_number: page_index + 1,
                logical_page_label: label_catalog.label_for_page(page_index),
                snippet,
                match_count,
                highlight_rects,
            });
        }

        Ok(SearchResponse {
            query: trimmed.to_owned(),
            hits,
            progress,
        })
    }

    /// Purges all disposable FTS5 rows and cached structured pages on Critical memory pressure (`ARCHITECTURE.md` §10).
    pub fn clear_for_memory_pressure(&mut self) -> Result<(), SearchIndexError> {
        self.conn
            .execute("DELETE FROM page_fts", [])
            .map_err(|err| SearchIndexError::SqliteFailure {
                details: err.to_string(),
            })?;
        self.indexed_pages.clear();
        self.structured_pages.clear();
        Ok(())
    }
}

/// Escapes user search input into a safe FTS5 phrase/prefix query.
fn build_fts5_phrase_query(raw: &str) -> String {
    let escaped = raw.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_engine::PdfTextLine;

    fn make_page(page_index: u32, label: &str, line_text: &str) -> PdfPageStructuredText {
        PdfPageStructuredText {
            page_index,
            embedded_page_label: Some(label.to_owned()),
            words: vec![],
            lines: vec![PdfTextLine {
                bbox: PdfRectMilliPt {
                    x0_milli_pt: 72_000,
                    y0_milli_pt: 700_000,
                    x1_milli_pt: 480_000,
                    y1_milli_pt: 718_000,
                },
                text: line_text.to_owned(),
                ends_paragraph: true,
            }],
        }
    }

    #[test]
    fn progressive_sqlite_fts5_returns_results_before_full_document_is_indexed() {
        let mut index = SessionSearchIndex::new(4).expect("create FTS5 index");
        let catalog = PageLabelCatalog::new(
            4,
            vec![
                Some("i".to_owned()),
                Some("ii".to_owned()),
                Some("1".to_owned()),
                Some("2".to_owned()),
            ],
        );

        assert_eq!(index.next_unindexed_pages(2, 2), vec![2, 3]);

        // Index ONLY page 2 (physical page 3, logical label "1") out of 4 pages:
        let prog_partial = index
            .index_structured_page(make_page(
                2,
                "1",
                "Chapter 1: Progressive SQLite FTS5 Search",
            ))
            .expect("index page 2");
        assert_eq!(prog_partial.indexed_pages, 1);
        assert_eq!(prog_partial.total_pages, 4);
        assert!(!prog_partial.is_complete);

        // M0.3 Gate: Search returns useful results BEFORE the document is fully indexed!
        let partial_search = index
            .search_exact("SQLite FTS5", &catalog)
            .expect("search partial index");
        assert!(!partial_search.progress.is_complete);
        assert_eq!(partial_search.hits.len(), 1);
        assert_eq!(partial_search.hits[0].page_index, 2);
        assert_eq!(partial_search.hits[0].physical_page_number, 3);
        assert_eq!(partial_search.hits[0].logical_page_label, "1");
        assert_eq!(partial_search.hits[0].highlight_rects.len(), 1);

        // Index remaining pages progressively:
        index
            .index_structured_page(make_page(0, "i", "Vocument Preface"))
            .expect("index page 0");
        index
            .index_structured_page(make_page(1, "ii", "Executive Summary"))
            .expect("index page 1");
        let prog_complete = index
            .index_structured_page(make_page(3, "2", "Chapter 2: Adaptive Thumbnail Cache"))
            .expect("index page 3");
        assert!(prog_complete.is_complete);
        assert_eq!(prog_complete.indexed_pages, 4);
    }
}
