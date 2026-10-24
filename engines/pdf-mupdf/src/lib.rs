//! MuPDF adapter behind Vocument's `PdfEngine` interface (`ARCHITECTURE.md` §§7, 9, 17, 18, `ROADMAP.md` M0.1–M0.3).
//!
//! No UI layer may depend on this crate directly.

pub mod attach;
pub mod comment;
pub mod geometry_ops;
pub mod inspect;
pub mod inspect_summary;
pub mod multidoc_ops;
pub mod mutate;
pub mod optimize_ops;
pub mod page_ops;
pub mod render_page;
pub mod sanitize;
pub mod scrub;
pub mod stamp_ops;
pub mod text_extract;

#[cfg(test)]
mod geometry_tests;
#[cfg(test)]
pub mod tests;

#[cfg(test)]
mod attachment_signature_tests;
#[cfg(test)]
mod comment_tests;
#[cfg(test)]
mod difficult_tests;
#[cfg(test)]
mod inspection_tests;
#[cfg(test)]
mod multidoc_tests;
#[cfg(test)]
mod redaction_image_tests;
#[cfg(test)]
mod redaction_rotation_tests;
#[cfg(test)]
mod sanitize_tests;
#[cfg(test)]
mod scrub_hardening_tests;
#[cfg(test)]
mod scrub_tests;
#[cfg(test)]
mod text_space_tests;
#[cfg(test)]
mod workbench_invariant_tests;

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint};
use std::ptr::NonNull;
use std::sync::Mutex;
use vocument_core::document_inspection::{PdfActionInfo, PdfAttachmentInfo, PdfSignatureInfo};
use vocument_core::pdf_engine::{
    BatesNumberingOptions, BookmarkItem, ExtractedImageData, ExtractedImageInfo,
    HeaderFooterOptions, OptimizationOptions, PageBoxUpdate, PdfCommentAnnotation,
    PdfDocumentSummary, PdfEngine, PdfEngineDisplayList, PdfEngineDocument, PdfEngineError,
    PdfHighlightAnnotation, PdfPageBoxes, PdfPageGeometry, PdfPageRaster, PdfPageStructuredText,
    PdfRectMilliPt, PdfTextOccurrence, PdfTileBitmap, PdfTileRenderRequest, PdfxPreflightReport,
    PrintPreflightReport, StampOrWatermarkOptions,
};

use inspect_summary::inspect_summary;

/// Identifier prefix of the PDF engine adapter.
pub const ENGINE_NAME: &str = "mupdf";

#[repr(C)]
struct RawMuPdfPageGeometry {
    status_code: c_int,
    width_milli_pt: c_uint,
    height_milli_pt: c_uint,
    error_message: [c_char; 512],
}

#[repr(C)]
struct RawMuPdfDisplayListHandle {
    _opaque: [u8; 0],
}

unsafe extern "C" {

    fn vocument_mupdf_query_page_geometry(
        bytes: *const c_uchar,
        len: usize,
        page_index: c_int,
        out_geom: *mut RawMuPdfPageGeometry,
    ) -> c_int;

    fn vocument_mupdf_create_display_list(
        bytes: *const c_uchar,
        len: usize,
        page_index: c_int,
        out_handle: *mut *mut RawMuPdfDisplayListHandle,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_render_tile_from_display_list(
        handle: *mut RawMuPdfDisplayListHandle,
        scale_milli: c_uint,
        tile_x_px: c_uint,
        tile_y_px: c_uint,
        tile_width_px: c_uint,
        tile_height_px: c_uint,
        out_rgba_bytes: *mut c_uchar,
        out_rgba_len: usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_drop_display_list(handle: *mut RawMuPdfDisplayListHandle);
}

pub(crate) fn c_buf_to_optional_string(buf: &[c_char]) -> Option<String> {
    // SAFETY: `buf` is zero-initialized and NUL-terminated by `mupdf_bridge.c`.
    let c_str = unsafe { CStr::from_ptr(buf.as_ptr()) };
    let text = c_str.to_string_lossy().trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}

/// Safe wrapper around the C `vocument_mupdf_display_list_handle_t` pointer.
struct OwnedDisplayListPtr(NonNull<RawMuPdfDisplayListHandle>);

// SAFETY: Access to the display list's dedicated `fz_context` is serialized by the `Mutex` in `MuPdfDisplayListHandle`.
unsafe impl Send for OwnedDisplayListPtr {}

impl Drop for OwnedDisplayListPtr {
    fn drop(&mut self) {
        // SAFETY: `self.0` was allocated by `vocument_mupdf_create_display_list`.
        unsafe {
            vocument_mupdf_drop_display_list(self.0.as_ptr());
        }
    }
}

/// Concrete MuPDF-backed `PdfEngineDisplayList` (`ARCHITECTURE.md` §7, `ROADMAP.md` M0.2).
pub struct MuPdfDisplayListHandle {
    geometry: PdfPageGeometry,
    raw_handle: Mutex<OwnedDisplayListPtr>,
}

impl std::fmt::Debug for MuPdfDisplayListHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MuPdfDisplayListHandle")
            .field("geometry", &self.geometry)
            .finish()
    }
}

impl PdfEngineDisplayList for MuPdfDisplayListHandle {
    fn page_geometry(&self) -> PdfPageGeometry {
        self.geometry
    }

    fn render_tile(&self, request: PdfTileRenderRequest) -> Result<PdfTileBitmap, PdfEngineError> {
        if request.tile_width_px == 0 || request.tile_height_px == 0 {
            return Err(PdfEngineError::InvalidTileDimensions {
                width: request.tile_width_px,
                height: request.tile_height_px,
            });
        }

        let byte_len = (request.tile_width_px as usize)
            .saturating_mul(request.tile_height_px as usize)
            .saturating_mul(4);
        let mut rgba_bytes = vec![255u8; byte_len];
        let mut err_buf = [0 as c_char; 512];

        let guard = self
            .raw_handle
            .lock()
            .map_err(|_| PdfEngineError::EngineFailure {
                details: "display-list mutex poisoned".to_owned(),
            })?;

        // SAFETY: `guard.0` is a valid non-null handle and `rgba_bytes` has `byte_len` capacity.
        let status = unsafe {
            vocument_mupdf_render_tile_from_display_list(
                guard.0.as_ptr(),
                request.scale_milli,
                request.tile_x_px,
                request.tile_y_px,
                request.tile_width_px,
                request.tile_height_px,
                rgba_bytes.as_mut_ptr(),
                rgba_bytes.len(),
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };

        if status != 0 {
            let details = c_buf_to_optional_string(&err_buf)
                .unwrap_or_else(|| "MuPDF failed to rasterize display-list tile".to_owned());
            return Err(PdfEngineError::EngineFailure { details });
        }

        Ok(PdfTileBitmap {
            width_px: request.tile_width_px,
            height_px: request.tile_height_px,
            stride_bytes: request.tile_width_px * 4,
            rgba_bytes,
        })
    }
}

/// Concrete MuPDF-backed `PdfEngineDocument` owned inside a `DocumentActor`.
#[derive(Debug)]
pub struct MuPdfDocumentHandle {
    summary: PdfDocumentSummary,
    pdf_bytes: Vec<u8>,
}

impl PdfEngineDocument for MuPdfDocumentHandle {
    fn summary(&self) -> &PdfDocumentSummary {
        &self.summary
    }

    fn current_pdf_bytes(&self) -> &[u8] {
        &self.pdf_bytes
    }

    fn page_geometry(&self, page_index: u32) -> Result<PdfPageGeometry, PdfEngineError> {
        if page_index >= self.summary.page_count {
            return Err(PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            });
        }

        let mut raw_geom = RawMuPdfPageGeometry {
            status_code: 0,
            width_milli_pt: 0,
            height_milli_pt: 0,
            error_message: [0; 512],
        };

        let page_idx_int =
            c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            })?;

        // SAFETY: `self.pdf_bytes` is a valid buffer and `raw_geom` is stack-allocated.
        let status = unsafe {
            vocument_mupdf_query_page_geometry(
                self.pdf_bytes.as_ptr(),
                self.pdf_bytes.len(),
                page_idx_int,
                &raw mut raw_geom,
            )
        };

        match status {
            0 => Ok(PdfPageGeometry {
                page_index,
                width_milli_pt: raw_geom.width_milli_pt,
                height_milli_pt: raw_geom.height_milli_pt,
            }),
            4 => Err(PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            }),
            _ => {
                let details = c_buf_to_optional_string(&raw_geom.error_message)
                    .unwrap_or_else(|| "failed to query PDF page geometry".to_owned());
                Err(PdfEngineError::EngineFailure { details })
            }
        }
    }

    fn page_rotation_degrees(&self, page_index: u32) -> Result<u32, PdfEngineError> {
        mutate::query_page_rotation_degrees(&self.pdf_bytes, self.summary.page_count, page_index)
    }

    fn page_highlight_annotations(
        &self,
        page_index: u32,
    ) -> Result<Vec<PdfHighlightAnnotation>, PdfEngineError> {
        mutate::query_page_highlights(&self.pdf_bytes, self.summary.page_count, page_index)
    }

    fn page_comment_annotations(
        &self,
        page_index: u32,
    ) -> Result<Vec<PdfCommentAnnotation>, PdfEngineError> {
        comment::query_page_comments(&self.pdf_bytes, self.summary.page_count, page_index)
    }

    fn add_comment_annotation(
        &mut self,
        annotation: &PdfCommentAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated = comment::add_comment_annotation_to_pdf(
            &self.pdf_bytes,
            self.summary.page_count,
            annotation,
        )?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn remove_comment_annotation(
        &mut self,
        page_index: u32,
        annotation_id: &str,
    ) -> Result<(PdfCommentAnnotation, Vec<u8>), PdfEngineError> {
        let (removed, updated) = comment::remove_comment_annotation_from_pdf(
            &self.pdf_bytes,
            self.summary.page_count,
            page_index,
            annotation_id,
        )?;
        self.pdf_bytes = updated.clone();
        Ok((removed, updated))
    }

    fn page_label(&self, page_index: u32) -> Result<Option<String>, PdfEngineError> {
        text_extract::query_embedded_page_label(
            &self.pdf_bytes,
            self.summary.page_count,
            page_index,
        )
    }

    fn extract_page_text(&self, page_index: u32) -> Result<PdfPageStructuredText, PdfEngineError> {
        text_extract::extract_structured_page_text(
            &self.pdf_bytes,
            self.summary.page_count,
            page_index,
        )
    }

    fn create_display_list(
        &self,
        page_index: u32,
    ) -> Result<Box<dyn PdfEngineDisplayList>, PdfEngineError> {
        if page_index >= self.summary.page_count {
            return Err(PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            });
        }

        let page_idx_int =
            c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            })?;

        let mut raw_ptr: *mut RawMuPdfDisplayListHandle = std::ptr::null_mut();
        let mut err_buf = [0 as c_char; 512];

        // SAFETY: `self.pdf_bytes` is valid and `raw_ptr` receives the allocated display list handle on status 0.
        let status = unsafe {
            vocument_mupdf_create_display_list(
                self.pdf_bytes.as_ptr(),
                self.pdf_bytes.len(),
                page_idx_int,
                &raw mut raw_ptr,
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };

        if status != 0 {
            let details = c_buf_to_optional_string(&err_buf)
                .unwrap_or_else(|| "MuPDF failed to create display list".to_owned());
            return Err(PdfEngineError::EngineFailure { details });
        }

        let non_null = NonNull::new(raw_ptr).ok_or_else(|| PdfEngineError::EngineFailure {
            details: "MuPDF returned null display list handle".to_owned(),
        })?;

        let geometry = self.page_geometry(page_index)?;
        Ok(Box::new(MuPdfDisplayListHandle {
            geometry,
            raw_handle: Mutex::new(OwnedDisplayListPtr(non_null)),
        }))
    }

    fn rotate_pages(
        &mut self,
        page_indices: &[u32],
        delta_degrees: i32,
    ) -> Result<Vec<u8>, PdfEngineError> {
        self.rotate_pages_ext(page_indices, delta_degrees, false)
    }

    fn rotate_pages_ext(
        &mut self,
        page_indices: &[u32],
        delta_degrees: i32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated = mutate::rotate_pages_in_pdf_ext(
            &self.pdf_bytes,
            self.summary.page_count,
            page_indices,
            delta_degrees,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn add_highlight_annotation(
        &mut self,
        annotation: &PdfHighlightAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated = mutate::add_highlight_annotation_to_pdf(
            &self.pdf_bytes,
            self.summary.page_count,
            annotation,
        )?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn remove_highlight_annotation(
        &mut self,
        page_index: u32,
        annotation_id: &str,
    ) -> Result<(PdfHighlightAnnotation, Vec<u8>), PdfEngineError> {
        let (removed, updated) = mutate::remove_highlight_annotation_from_pdf(
            &self.pdf_bytes,
            self.summary.page_count,
            page_index,
            annotation_id,
        )?;
        self.pdf_bytes = updated.clone();
        Ok((removed, updated))
    }

    fn redact_page_regions(
        &mut self,
        page_index: u32,
        regions: &[PdfRectMilliPt],
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated = mutate::redact_page_regions_in_pdf(
            &self.pdf_bytes,
            self.summary.page_count,
            page_index,
            regions,
        )?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn render_page_gray(
        &self,
        page_index: u32,
        scale_milli: u32,
    ) -> Result<PdfPageRaster, PdfEngineError> {
        render_page::render_page_gray(
            &self.pdf_bytes,
            self.summary.page_count,
            page_index,
            scale_milli,
        )
    }

    fn find_word_occurrences(
        &self,
        words: &[String],
    ) -> Result<Vec<PdfTextOccurrence>, PdfEngineError> {
        scrub::find_word_occurrences(&self.pdf_bytes, words)
    }

    fn replace_pdf_bytes(&mut self, bytes: Vec<u8>) -> Result<(), PdfEngineError> {
        if bytes.is_empty() {
            return Err(PdfEngineError::EngineFailure {
                details: "the document must not become empty".to_owned(),
            });
        }
        self.summary = inspect_summary(&bytes)?;
        self.pdf_bytes = bytes;
        Ok(())
    }

    fn document_actions(&self) -> Result<Vec<PdfActionInfo>, PdfEngineError> {
        inspect::document_actions(&self.pdf_bytes)
    }

    fn document_attachments(&self) -> Result<Vec<PdfAttachmentInfo>, PdfEngineError> {
        attach::document_attachments(&self.pdf_bytes)
    }

    fn document_signatures(&self) -> Result<Vec<PdfSignatureInfo>, PdfEngineError> {
        attach::document_signatures(&self.pdf_bytes)
    }

    fn sanitize_document(
        &mut self,
        options: &vocument_core::pdf_engine::PdfSanitizeOptions,
    ) -> Result<(vocument_core::pdf_engine::PdfSanitizeReport, Vec<u8>), PdfEngineError> {
        let (sanitized_bytes, report) = sanitize::sanitize_document(&self.pdf_bytes, options)?;
        self.summary = inspect_summary(&sanitized_bytes)?;
        self.pdf_bytes = sanitized_bytes.clone();
        Ok((report, sanitized_bytes))
    }

    fn reorder_pages(&mut self, order: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
        self.reorder_pages_ext(order, false)
    }

    fn reorder_pages_ext(
        &mut self,
        order: &[u32],
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let new_bytes =
            page_ops::reorder_pages_ext(&self.pdf_bytes, order, allow_signature_invalidation)?;
        self.summary = inspect_summary(&new_bytes)?;
        self.pdf_bytes = new_bytes.clone();
        Ok(new_bytes)
    }

    fn duplicate_page(&mut self, page_index: u32) -> Result<Vec<u8>, PdfEngineError> {
        self.duplicate_page_ext(page_index, false)
    }

    fn duplicate_page_ext(
        &mut self,
        page_index: u32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let new_bytes = page_ops::duplicate_page_ext(
            &self.pdf_bytes,
            page_index,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&new_bytes)?;
        self.pdf_bytes = new_bytes.clone();
        Ok(new_bytes)
    }

    fn delete_pages(&mut self, page_indices: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
        self.delete_pages_ext(page_indices, false)
    }

    fn delete_pages_ext(
        &mut self,
        page_indices: &[u32],
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let new_bytes = page_ops::delete_pages_ext(
            &self.pdf_bytes,
            page_indices,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&new_bytes)?;
        self.pdf_bytes = new_bytes.clone();
        Ok(new_bytes)
    }

    fn extract_pages(&self, page_indices: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
        page_ops::extract_pages(&self.pdf_bytes, page_indices)
    }

    fn merge_documents_ext(
        &mut self,
        source_documents: &[&[u8]],
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let mut all_docs = Vec::with_capacity(1 + source_documents.len());
        all_docs.push(self.pdf_bytes.as_slice());
        all_docs.extend_from_slice(source_documents);

        let merged_bytes =
            multidoc_ops::merge_documents_ext(&all_docs, allow_signature_invalidation)?;
        self.summary = inspect_summary(&merged_bytes)?;
        self.pdf_bytes = merged_bytes.clone();
        Ok(merged_bytes)
    }

    fn insert_pages_ext(
        &mut self,
        source_pdf: &[u8],
        source_page_indices: &[u32],
        insert_at: u32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated_bytes = multidoc_ops::insert_pages_ext(
            &self.pdf_bytes,
            source_pdf,
            source_page_indices,
            insert_at,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated_bytes)?;
        self.pdf_bytes = updated_bytes.clone();
        Ok(updated_bytes)
    }

    fn split_document(&self, split_ranges: &[Vec<u32>]) -> Result<Vec<Vec<u8>>, PdfEngineError> {
        multidoc_ops::split_document(&self.pdf_bytes, split_ranges)
    }

    fn get_page_boxes(&self, page_index: u32) -> Result<PdfPageBoxes, PdfEngineError> {
        let boxes = geometry_ops::get_page_boxes(&self.pdf_bytes, page_index as usize)?;
        let mb = boxes.media_box;
        Ok(PdfPageBoxes {
            media_box: geometry_ops::pdf_points_to_milli_pt_rect(boxes.media_box, mb),
            crop_box: geometry_ops::pdf_points_to_milli_pt_rect(boxes.crop_box, mb),
            bleed_box: geometry_ops::pdf_points_to_milli_pt_rect(boxes.bleed_box, mb),
            trim_box: geometry_ops::pdf_points_to_milli_pt_rect(boxes.trim_box, mb),
            art_box: geometry_ops::pdf_points_to_milli_pt_rect(boxes.art_box, mb),
            rotation_degrees: boxes.rotation_degrees,
        })
    }

    fn set_page_boxes_ext(
        &mut self,
        page_indices: &[u32],
        boxes: &PageBoxUpdate,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        if page_indices.is_empty() {
            return Err(PdfEngineError::InvalidPdf {
                details: "No page indices provided to set_page_boxes".to_owned(),
            });
        }
        let cur_boxes = geometry_ops::get_page_boxes(&self.pdf_bytes, page_indices[0] as usize)?;
        let mb = cur_boxes.media_box;

        let m_box = boxes
            .media_box
            .map(|r| geometry_ops::milli_pt_rect_to_pdf_points(r, mb));
        let c_box = boxes
            .crop_box
            .map(|r| geometry_ops::milli_pt_rect_to_pdf_points(r, mb));
        let b_box = boxes
            .bleed_box
            .map(|r| geometry_ops::milli_pt_rect_to_pdf_points(r, mb));
        let t_box = boxes
            .trim_box
            .map(|r| geometry_ops::milli_pt_rect_to_pdf_points(r, mb));
        let a_box = boxes
            .art_box
            .map(|r| geometry_ops::milli_pt_rect_to_pdf_points(r, mb));

        let indices: Vec<usize> = page_indices.iter().map(|&i| i as usize).collect();
        let updated = geometry_ops::set_page_boxes_ext(
            &self.pdf_bytes,
            &indices,
            m_box,
            c_box,
            b_box,
            t_box,
            a_box,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn apply_hard_crop_ext(
        &mut self,
        page_indices: &[u32],
        crop_rect: PdfRectMilliPt,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        if page_indices.is_empty() {
            return Err(PdfEngineError::InvalidPdf {
                details: "No page indices provided for hard crop".to_owned(),
            });
        }
        let cur_boxes = geometry_ops::get_page_boxes(&self.pdf_bytes, page_indices[0] as usize)?;
        let crop_pts = geometry_ops::milli_pt_rect_to_pdf_points(crop_rect, cur_boxes.media_box);
        let indices: Vec<usize> = page_indices.iter().map(|&i| i as usize).collect();

        let updated = geometry_ops::apply_hard_crop_ext(
            &self.pdf_bytes,
            &indices,
            crop_pts,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn detect_deskew_angle(&self, page_index: u32) -> Result<f32, PdfEngineError> {
        geometry_ops::detect_deskew_angle(&self.pdf_bytes, page_index as usize)
    }

    fn apply_deskew_ext(
        &mut self,
        page_indices: &[u32],
        angle_degrees: f32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let indices: Vec<usize> = page_indices.iter().map(|&i| i as usize).collect();
        let updated = geometry_ops::apply_deskew_ext(
            &self.pdf_bytes,
            &indices,
            angle_degrees,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn detect_content_margins(
        &self,
        page_index: u32,
        safety_margin_milli_pt: i32,
    ) -> Result<PdfRectMilliPt, PdfEngineError> {
        let cur_boxes = geometry_ops::get_page_boxes(&self.pdf_bytes, page_index as usize)?;
        let safety_pt = safety_margin_milli_pt as f32 / 1000.0;
        let pts =
            geometry_ops::detect_content_margins(&self.pdf_bytes, page_index as usize, safety_pt)?;
        Ok(geometry_ops::pdf_points_to_milli_pt_rect(
            pts,
            cur_boxes.media_box,
        ))
    }

    fn apply_auto_crop_ext(
        &mut self,
        page_indices: &[u32],
        safety_margin_milli_pt: i32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let safety_pt = safety_margin_milli_pt as f32 / 1000.0;
        let indices: Vec<usize> = page_indices.iter().map(|&i| i as usize).collect();

        let updated = geometry_ops::apply_auto_crop_ext(
            &self.pdf_bytes,
            &indices,
            safety_pt,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn apply_bates_numbering_ext(
        &mut self,
        page_indices: &[u32],
        options: &BatesNumberingOptions,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated = stamp_ops::apply_bates_numbering_ext(
            &self.pdf_bytes,
            page_indices,
            options,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn apply_header_footer_ext(
        &mut self,
        page_indices: &[u32],
        options: &HeaderFooterOptions,
        doc_title: Option<&str>,
        date_str: Option<&str>,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated = stamp_ops::apply_header_footer_ext(
            &self.pdf_bytes,
            page_indices,
            options,
            doc_title,
            date_str,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn get_bookmarks(&self) -> Result<Vec<BookmarkItem>, PdfEngineError> {
        stamp_ops::get_bookmarks(&self.pdf_bytes)
    }

    fn set_bookmarks_ext(
        &mut self,
        bookmarks: &[BookmarkItem],
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated =
            stamp_ops::set_bookmarks_ext(&self.pdf_bytes, bookmarks, allow_signature_invalidation)?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn apply_stamp_or_watermark_ext(
        &mut self,
        page_indices: &[u32],
        options: &StampOrWatermarkOptions,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let updated = stamp_ops::apply_stamp_or_watermark_ext(
            &self.pdf_bytes,
            page_indices,
            options,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn optimize_document_ext(
        &mut self,
        options: &OptimizationOptions,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let (updated, _) = optimize_ops::optimize_document_ext(
            &self.pdf_bytes,
            options,
            allow_signature_invalidation,
        )?;
        self.summary = inspect_summary(&updated)?;
        self.pdf_bytes = updated.clone();
        Ok(updated)
    }

    fn get_document_images(&self) -> Result<Vec<ExtractedImageInfo>, PdfEngineError> {
        optimize_ops::get_document_images(&self.pdf_bytes)
    }

    fn extract_image_bytes(
        &self,
        page_index: u32,
        image_index: u32,
    ) -> Result<ExtractedImageData, PdfEngineError> {
        optimize_ops::extract_image_bytes(&self.pdf_bytes, page_index, image_index)
    }

    fn preflight_pdfx(&self) -> Result<PdfxPreflightReport, PdfEngineError> {
        optimize_ops::preflight_pdfx(&self.pdf_bytes)
    }

    fn preflight_print(&self) -> Result<PrintPreflightReport, PdfEngineError> {
        optimize_ops::preflight_print(&self.pdf_bytes)
    }
}

/// Production `PdfEngine` adapter backed by MuPDF (`ARCHITECTURE.md` §18).
#[derive(Debug, Default, Clone, Copy)]
pub struct MuPdfEngineAdapter;

impl MuPdfEngineAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PdfEngine for MuPdfEngineAdapter {
    fn engine_identifier(&self) -> &str {
        ENGINE_NAME
    }

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        let summary = inspect_summary(bytes)?;
        Ok(Box::new(MuPdfDocumentHandle {
            summary,
            pdf_bytes: bytes.to_vec(),
        }))
    }
}
