use super::*;
use crate::file_broker::{BrokeredFileRead, FileAccessOrigin};
use crate::pdf_engine::{
    PdfCommentAnnotation, PdfDocumentSummary, PdfEngine, PdfEngineDisplayList, PdfEngineDocument,
    PdfEngineError, PdfPageGeometry, PdfPageStructuredText, PdfRectMilliPt, PdfTileBitmap,
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

    fn render_tile(&self, request: PdfTileRenderRequest) -> Result<PdfTileBitmap, PdfEngineError> {
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
        self.rotations
            .get(page_index as usize)
            .copied()
            .ok_or(PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            })
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

    /// The comments of the page (`ANNOT-001`). The test engine does not carry
    /// comments — it is a stand-in for the real engine, and it says so
    /// instead of pretending that it succeeded.
    fn page_comment_annotations(
        &self,
        _page_index: u32,
    ) -> Result<Vec<PdfCommentAnnotation>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    /// Writes a comment (`ANNOT-001`).
    fn add_comment_annotation(
        &mut self,
        _annotation: &PdfCommentAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    /// Removes a comment (`ANNOT-001`).
    fn remove_comment_annotation(
        &mut self,
        _page_index: u32,
        _annotation_id: &str,
    ) -> Result<(PdfCommentAnnotation, Vec<u8>), PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    fn page_label(&self, _page_index: u32) -> Result<Option<String>, PdfEngineError> {
        Ok(None)
    }

    fn extract_page_text(&self, page_index: u32) -> Result<PdfPageStructuredText, PdfEngineError> {
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

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
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
fn transaction_manager_executes_highlight_and_rotation_undo_redo_and_persistent_journal_replay() {
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
