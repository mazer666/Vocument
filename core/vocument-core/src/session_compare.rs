//! SessionRegistry extensions for M3.7 Document Comparison & Diffing (`COMPARE-001`–`COMPARE-004`).

use crate::compare::{
    ComparisonOptions, ComparisonReport, PageDiffResult, compare_documents, compare_pages_visual,
};
use crate::session::{SessionError, SessionRegistry};

impl SessionRegistry {
    /// Compares two active document sessions structurally and visually (`COMPARE-001`–`COMPARE-004`).
    pub fn compare_sessions(
        &self,
        session_id_a: &str,
        session_id_b: &str,
        options: &ComparisonOptions,
    ) -> Result<ComparisonReport, SessionError> {
        let actor_a = self.session_actor_and_meta(session_id_a)?.0;
        let actor_b = self.session_actor_and_meta(session_id_b)?.0;

        if session_id_a == session_id_b {
            return actor_a.with_engine_document(|doc| {
                compare_documents(doc, doc, options).map_err(SessionError::Compare)
            });
        }

        // Enforce strict deterministic lock ordering by session_id to prevent deadlocks
        if session_id_a < session_id_b {
            actor_a.with_engine_document(|doc_a| {
                actor_b.with_engine_document(|doc_b| {
                    compare_documents(doc_a, doc_b, options).map_err(SessionError::Compare)
                })
            })
        } else {
            actor_b.with_engine_document(|doc_b| {
                actor_a.with_engine_document(|doc_a| {
                    compare_documents(doc_a, doc_b, options).map_err(SessionError::Compare)
                })
            })
        }
    }

    /// Compares two specific pages visually across sessions (`COMPARE-002`).
    pub fn compare_pages_visual(
        &self,
        session_id_a: &str,
        page_a: u32,
        session_id_b: &str,
        page_b: u32,
        options: &ComparisonOptions,
    ) -> Result<PageDiffResult, SessionError> {
        let actor_a = self.session_actor_and_meta(session_id_a)?.0;
        let actor_b = self.session_actor_and_meta(session_id_b)?.0;

        if session_id_a == session_id_b {
            return actor_a.with_engine_document(|doc| {
                compare_pages_visual(doc, page_a, doc, page_b, options)
                    .map_err(SessionError::Compare)
            });
        }

        if session_id_a < session_id_b {
            actor_a.with_engine_document(|doc_a| {
                actor_b.with_engine_document(|doc_b| {
                    compare_pages_visual(doc_a, page_a, doc_b, page_b, options)
                        .map_err(SessionError::Compare)
                })
            })
        } else {
            actor_b.with_engine_document(|doc_b| {
                actor_a.with_engine_document(|doc_a| {
                    compare_pages_visual(doc_a, page_a, doc_b, page_b, options)
                        .map_err(SessionError::Compare)
                })
            })
        }
    }
}
