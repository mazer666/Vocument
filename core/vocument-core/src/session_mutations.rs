//! M0.4 mutation, Undo/Redo, and persistent transaction journal operations on `SessionRegistry` (`ROADMAP.md` M0.4, `ARCHITECTURE.md` §11).
//!
//! Gate (`ROADMAP.md` M0.4): **Mutation never bypasses `TransactionManager`.**

use crate::pdf_engine::{PdfHighlightAnnotation, PdfRectMilliPt};
use crate::session::{SessionError, SessionRegistry};
use crate::transactions::{
    PersistentTransactionJournal, TransactionCommand, TransactionHistorySnapshot, TransactionRecord,
};

impl SessionRegistry {
    /// Queries the current rotation (`0`, `90`, `180`, or `270` degrees) of `page_index` (`ROTATE-001`).
    pub fn page_rotation_degrees(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<u32, SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        Ok(actor.page_rotation_degrees(page_index)?)
    }

    /// Queries all standard PDF `/Highlight` annotations on `page_index` (`ANNOT-001`, `INTEROP-002`).
    pub fn page_highlight_annotations(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<Vec<PdfHighlightAnnotation>, SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        Ok(actor.page_highlight_annotations(page_index)?)
    }

    /// Executes a semantic `TransactionCommand` exclusively through `TransactionManager` (`ROADMAP.md` M0.4 Gate).
    pub fn execute_transaction(
        &self,
        session_id: &str,
        command: TransactionCommand,
    ) -> Result<TransactionRecord, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let record = session
            .transaction_manager
            .execute_command(&actor, command)?;
        session.sync_from_transaction_manager();
        drop(guard);
        let _ = self.record_recovery_mutation(session_id);
        Ok(record)
    }

    /// Rotates one or more `0`-based pages by `delta_degrees` (`+90`, `-90`, `180`, `-180`, `270`) via `TransactionManager` (`ROTATE-001`).
    pub fn rotate_pages(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        delta_degrees: i32,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(
            session_id,
            TransactionCommand::RotatePages {
                page_indices,
                delta_degrees,
            },
        )
    }

    /// Adds a standard PDF `/Highlight` annotation via `TransactionManager` (`ANNOT-001`, `INTEROP-002`).
    pub fn add_highlight_annotation(
        &self,
        session_id: &str,
        annotation: PdfHighlightAnnotation,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(
            session_id,
            TransactionCommand::AddHighlightAnnotation { annotation },
        )
    }

    /// Removes the content of the given regions of a page for good (`REDACT-001`).
    ///
    /// The redaction cannot be undone. The checkpoints of the recovery store
    /// still hold the bytes of the earlier states of the document, and those
    /// bytes still contain the removed content — they are deleted with the
    /// redaction.
    ///
    /// The checkpoints are **not** deleted here: the removal can still fail
    /// its verification, and then the document is put back. The checkpoints
    /// are deleted after the verification has succeeded.
    pub fn apply_redaction(
        &self,
        session_id: &str,
        page_index: u32,
        regions: Vec<PdfRectMilliPt>,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(
            session_id,
            TransactionCommand::RedactPageRegions {
                page_index,
                regions,
            },
        )
    }

    /// Writes the current text of a page into the search index.
    ///
    /// After a redaction the index has to be updated, otherwise the search
    /// would still find the text that was removed from the document
    /// (`REDACT-001`).
    pub(crate) fn reindex_page(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<(), SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        let structured = actor.extract_page_text(page_index)?;

        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let _ = session.search_index.index_structured_page(structured)?;
        Ok(())
    }

    /// Removes a standard PDF `/Highlight` annotation by `annotation_id` via `TransactionManager` (`ANNOT-001`).
    pub fn remove_highlight_annotation(
        &self,
        session_id: &str,
        page_index: u32,
        annotation_id: String,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(
            session_id,
            TransactionCommand::RemoveHighlightAnnotation {
                page_index,
                annotation_id,
                restored_annotation: None,
            },
        )
    }

    /// Undoes the most recent semantic transaction on `session_id` (`UNDO-001`, `UNDO-004`).
    pub fn undo(&self, session_id: &str) -> Result<TransactionRecord, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let record = session.transaction_manager.undo(&actor)?;
        session.sync_from_transaction_manager();
        drop(guard);
        let _ = self.record_recovery_mutation(session_id);
        Ok(record)
    }

    /// Redoes the most recently undone semantic transaction on `session_id` (`UNDO-001`, `UNDO-004`).
    pub fn redo(&self, session_id: &str) -> Result<TransactionRecord, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let record = session.transaction_manager.redo(&actor)?;
        session.sync_from_transaction_manager();
        drop(guard);
        let _ = self.record_recovery_mutation(session_id);
        Ok(record)
    }

    /// Queries the live Undo/Redo history snapshot of `session_id` (`ROADMAP.md` M0.4).
    pub fn transaction_history(
        &self,
        session_id: &str,
    ) -> Result<TransactionHistorySnapshot, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        Ok(session.transaction_manager.history_snapshot())
    }

    /// Exports the session's `PersistentTransactionJournal` as checksum-verified pretty JSON (`ARCHITECTURE.md` §§11–12, `UNDO-001`).
    pub fn export_persistent_transaction_journal_json(
        &self,
        session_id: &str,
    ) -> Result<String, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let journal = session.transaction_manager.export_persistent_journal();
        Ok(journal.to_json_pretty()?)
    }

    /// Verifies and replays a `PersistentTransactionJournal` JSON payload onto `session_id` (`UNDO-001`).
    pub fn restore_persistent_transaction_journal_json(
        &self,
        session_id: &str,
        journal_json: &str,
    ) -> Result<TransactionHistorySnapshot, SessionError> {
        let journal = PersistentTransactionJournal::from_json_verified(journal_json)?;
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let snapshot = session
            .transaction_manager
            .restore_and_replay_journal(&actor, &journal)?;
        session.sync_from_transaction_manager();
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_broker::{BrokeredFileRead, FileAccessOrigin};
    use crate::pdf_engine::{
        PdfDocumentSummary, PdfEngine, PdfEngineDisplayList, PdfEngineDocument, PdfEngineError,
        PdfPageGeometry, PdfPageStructuredText, PdfRectMilliPt, PdfTileBitmap,
        PdfTileRenderRequest,
    };

    #[derive(Debug)]
    struct MockMutatingPdfDoc {
        summary: PdfDocumentSummary,
        bytes: Vec<u8>,
        rotations: Vec<u32>,
        highlights: Vec<PdfHighlightAnnotation>,
    }

    #[derive(Debug)]
    struct MockDisplayList {
        geometry: PdfPageGeometry,
    }

    impl PdfEngineDisplayList for MockDisplayList {
        fn page_geometry(&self) -> PdfPageGeometry {
            self.geometry
        }

        fn render_tile(
            &self,
            request: PdfTileRenderRequest,
        ) -> Result<PdfTileBitmap, PdfEngineError> {
            let count = (request.tile_width_px as usize) * (request.tile_height_px as usize) * 4;
            Ok(PdfTileBitmap {
                width_px: request.tile_width_px,
                height_px: request.tile_height_px,
                stride_bytes: request.tile_width_px * 4,
                rgba_bytes: vec![255; count],
            })
        }
    }

    impl PdfEngineDocument for MockMutatingPdfDoc {
        fn summary(&self) -> &PdfDocumentSummary {
            &self.summary
        }

        fn current_pdf_bytes(&self) -> &[u8] {
            &self.bytes
        }

        fn page_geometry(&self, page_index: u32) -> Result<PdfPageGeometry, PdfEngineError> {
            let rot = self.page_rotation_degrees(page_index)?;
            if rot == 90 || rot == 270 {
                Ok(PdfPageGeometry {
                    page_index,
                    width_milli_pt: 842_000,
                    height_milli_pt: 595_000,
                })
            } else {
                Ok(PdfPageGeometry {
                    page_index,
                    width_milli_pt: 595_000,
                    height_milli_pt: 842_000,
                })
            }
        }

        fn page_rotation_degrees(&self, page_index: u32) -> Result<u32, PdfEngineError> {
            self.rotations.get(page_index as usize).copied().ok_or(
                PdfEngineError::PageOutOfBounds {
                    page_index,
                    page_count: self.summary.page_count,
                },
            )
        }

        fn page_highlight_annotations(
            &self,
            page_index: u32,
        ) -> Result<Vec<PdfHighlightAnnotation>, PdfEngineError> {
            if page_index >= self.summary.page_count {
                return Err(PdfEngineError::PageOutOfBounds {
                    page_index,
                    page_count: self.summary.page_count,
                });
            }
            Ok(self
                .highlights
                .iter()
                .filter(|h| h.page_index == page_index)
                .cloned()
                .collect())
        }

        fn page_label(&self, _page_index: u32) -> Result<Option<String>, PdfEngineError> {
            Ok(None)
        }

        fn extract_page_text(
            &self,
            page_index: u32,
        ) -> Result<PdfPageStructuredText, PdfEngineError> {
            Ok(PdfPageStructuredText {
                page_index,
                embedded_page_label: None,
                words: vec![],
                lines: Vec::new(),
            })
        }

        fn create_display_list(
            &self,
            page_index: u32,
        ) -> Result<Box<dyn PdfEngineDisplayList>, PdfEngineError> {
            Ok(Box::new(MockDisplayList {
                geometry: self.page_geometry(page_index)?,
            }))
        }

        fn rotate_pages(
            &mut self,
            page_indices: &[u32],
            delta_degrees: i32,
        ) -> Result<Vec<u8>, PdfEngineError> {
            if delta_degrees % 90 != 0 {
                return Err(PdfEngineError::InvalidRotationAngle {
                    degrees: delta_degrees,
                });
            }
            for &idx in page_indices {
                let cur = self.page_rotation_degrees(idx)? as i32;
                let next = (cur + delta_degrees).rem_euclid(360) as u32;
                self.rotations[idx as usize] = next;
            }
            self.bytes = format!(
                "%PDF-1.7\nrot={:?}|annots={}",
                self.rotations,
                self.highlights.len()
            )
            .into_bytes();
            Ok(self.bytes.clone())
        }

        fn add_highlight_annotation(
            &mut self,
            annotation: &PdfHighlightAnnotation,
        ) -> Result<Vec<u8>, PdfEngineError> {
            self.highlights.push(annotation.clone());
            self.bytes = format!(
                "%PDF-1.7\nrot={:?}|annots={}",
                self.rotations,
                self.highlights.len()
            )
            .into_bytes();
            Ok(self.bytes.clone())
        }

        fn remove_highlight_annotation(
            &mut self,
            page_index: u32,
            annotation_id: &str,
        ) -> Result<(PdfHighlightAnnotation, Vec<u8>), PdfEngineError> {
            let pos = self
                .highlights
                .iter()
                .position(|h| h.page_index == page_index && h.annotation_id == annotation_id)
                .ok_or_else(|| PdfEngineError::AnnotationNotFound {
                    page_index,
                    annotation_id: annotation_id.to_owned(),
                })?;
            let removed = self.highlights.remove(pos);
            self.bytes = format!(
                "%PDF-1.7\nrot={:?}|annots={}",
                self.rotations,
                self.highlights.len()
            )
            .into_bytes();
            Ok((removed, self.bytes.clone()))
        }
    }

    #[derive(Debug)]
    struct MockMutatingEngine;

    impl PdfEngine for MockMutatingEngine {
        fn engine_identifier(&self) -> &str {
            "mock-m0.4"
        }

        fn open_pdf_bytes(
            &self,
            bytes: &[u8],
        ) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
            Ok(Box::new(MockMutatingPdfDoc {
                summary: PdfDocumentSummary {
                    page_count: 2,
                    format_version: Some("PDF 1.7".to_owned()),
                    title: Some("M0.4 Test".to_owned()),
                    author: None,
                    subject: None,
                    keywords: None,
                    creator: None,
                    producer: None,
                    creation_date: None,
                    modification_date: None,
                    needs_password: false,
                    engine_identifier: "mock-m0.4".to_owned(),
                },
                bytes: bytes.to_vec(),
                rotations: vec![0, 0],
                highlights: Vec::new(),
            }))
        }
    }

    #[test]
    fn transaction_manager_executes_highlight_and_rotation_undo_redo_and_persistent_journal_replay()
    {
        let registry = SessionRegistry::new(Box::new(MockMutatingEngine));
        let base_bytes = b"%PDF-1.7\nbase".to_vec();
        let snap = registry
            .open_brokered_file(BrokeredFileRead {
                source_token: "/tmp/m0_4.pdf".to_owned(),
                display_name: "m0_4.pdf".to_owned(),
                access_origin: FileAccessOrigin::MacOsScopedUrl,
                os_file_id: Some("dev:1:ino:4".to_owned()),
                modified_unix_ms: Some(1_760_000_000_000),
                bytes: base_bytes.clone(),
            })
            .expect("open session");

        let sid = snap.session_id.as_str();
        let annot = PdfHighlightAnnotation {
            annotation_id: "annot_0001".to_owned(),
            page_index: 0,
            rect: PdfRectMilliPt {
                x0_milli_pt: 72_000,
                y0_milli_pt: 100_000,
                x1_milli_pt: 300_000,
                y1_milli_pt: 120_000,
            },
            color_rgb: [255, 230, 80],
            opacity_milli: 450,
            author: Some("Jurist Preset".to_owned()),
            contents: Some("Key obligation".to_owned()),
        };

        let tx1 = registry
            .add_highlight_annotation(sid, annot)
            .expect("add highlight");
        assert_eq!(tx1.sequence_number, 1);
        assert_eq!(tx1.description, "Add highlight annotation on page #1");
        assert_eq!(
            registry
                .page_highlight_annotations(sid, 0)
                .expect("annots")
                .len(),
            1
        );

        let tx2 = registry
            .rotate_pages(sid, vec![0], 90)
            .expect("rotate page 0 +90");
        assert_eq!(tx2.sequence_number, 2);
        assert_eq!(tx2.description, "Rotate page #1 by +90°");
        assert_eq!(registry.page_rotation_degrees(sid, 0).expect("rot"), 90);

        // Undo rotation:
        let undone_rot = registry.undo(sid).expect("undo rotation");
        assert_eq!(undone_rot.transaction_id, tx2.transaction_id);
        assert_eq!(registry.page_rotation_degrees(sid, 0).expect("rot"), 0);

        // Undo highlight:
        let _ = registry.undo(sid).expect("undo highlight");
        assert!(
            registry
                .page_highlight_annotations(sid, 0)
                .expect("annots")
                .is_empty()
        );
        let clean_snap = registry.session_snapshot(sid).expect("snapshot");
        assert!(!clean_snap.is_dirty);

        // Redo both:
        let _ = registry.redo(sid).expect("redo highlight");
        let _ = registry.redo(sid).expect("redo rotation");
        assert_eq!(
            registry
                .page_highlight_annotations(sid, 0)
                .expect("annots")
                .len(),
            1
        );
        assert_eq!(registry.page_rotation_degrees(sid, 0).expect("rot"), 90);

        // Export persistent transaction journal JSON and replay onto a fresh session:
        let journal_json = registry
            .export_persistent_transaction_journal_json(sid)
            .expect("export journal JSON");
        assert!(journal_json.contains("\"schema_version\": 1"));

        let snap2 = registry
            .open_brokered_file(BrokeredFileRead {
                source_token: "/tmp/m0_4.pdf".to_owned(),
                display_name: "m0_4.pdf".to_owned(),
                access_origin: FileAccessOrigin::MacOsScopedUrl,
                os_file_id: Some("dev:1:ino:4".to_owned()),
                modified_unix_ms: Some(1_760_000_000_000),
                bytes: base_bytes,
            })
            .expect("open second session");
        let replayed = registry
            .restore_persistent_transaction_journal_json(snap2.session_id.as_str(), &journal_json)
            .expect("replay persistent journal");
        assert_eq!(replayed.undo_count, 2);
        assert!(replayed.is_dirty);
        assert_eq!(
            registry
                .page_rotation_degrees(snap2.session_id.as_str(), 0)
                .expect("replayed rotation"),
            90
        );
        assert_eq!(
            registry
                .page_highlight_annotations(snap2.session_id.as_str(), 0)
                .expect("replayed highlights")
                .len(),
            1
        );
    }
}
