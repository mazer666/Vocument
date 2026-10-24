//! Execution of semantic `TransactionCommand` variants against a `DocumentActor` (`ARCHITECTURE.md` §§6, 11).

use crate::pdf_engine::PdfEngineDocument;
use crate::render::DocumentActor;
use crate::transactions::{TransactionCommand, TransactionError};

/// Applies `command` to `actor`, returning the committed forward command, inverse command, and updated bytes.
pub fn apply_command_on_actor(
    actor: &DocumentActor,
    command: TransactionCommand,
) -> Result<(TransactionCommand, TransactionCommand, Vec<u8>), TransactionError> {
    match command {
        TransactionCommand::RotatePages {
            page_indices,
            delta_degrees,
            allow_signature_invalidation,
        } => {
            if page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                |doc: &mut dyn PdfEngineDocument| {
                    doc.rotate_pages_ext(&page_indices, delta_degrees, allow_signature_invalidation)
                },
            )?;
            let forward = TransactionCommand::RotatePages {
                page_indices: page_indices.clone(),
                delta_degrees,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RotatePages {
                page_indices,
                delta_degrees: -delta_degrees,
                allow_signature_invalidation: true,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::AddHighlightAnnotation { annotation } => {
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                |doc: &mut dyn PdfEngineDocument| doc.add_highlight_annotation(&annotation),
            )?;
            let inverse = TransactionCommand::RemoveHighlightAnnotation {
                page_index: annotation.page_index,
                annotation_id: annotation.annotation_id.clone(),
                restored_annotation: Some(annotation.clone()),
            };
            let forward = TransactionCommand::AddHighlightAnnotation { annotation };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::AddCommentAnnotation { annotation } => {
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                |doc: &mut dyn PdfEngineDocument| doc.add_comment_annotation(&annotation),
            )?;
            let inverse = TransactionCommand::RemoveCommentAnnotation {
                page_index: annotation.page_index,
                annotation_id: annotation.annotation_id.clone(),
                restored_annotation: Some(annotation.clone()),
            };
            let forward = TransactionCommand::AddCommentAnnotation { annotation };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::RemoveCommentAnnotation {
            page_index,
            annotation_id,
            ..
        } => {
            let (removed_annot, updated_bytes) = actor
                .apply_engine_mutation_for_transaction_manager(
                    |doc: &mut dyn PdfEngineDocument| {
                        doc.remove_comment_annotation(page_index, &annotation_id)
                    },
                )?;
            let forward = TransactionCommand::RemoveCommentAnnotation {
                page_index,
                annotation_id,
                restored_annotation: Some(removed_annot.clone()),
            };
            let inverse = TransactionCommand::AddCommentAnnotation {
                annotation: removed_annot,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::RedactPageRegions {
            page_index,
            regions,
        } => {
            if regions.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                |doc: &mut dyn PdfEngineDocument| doc.redact_page_regions(page_index, &regions),
            )?;
            let forward = TransactionCommand::RedactPageRegions {
                page_index,
                regions: regions.clone(),
            };
            let inverse = TransactionCommand::Irreversible {
                description: format!("The content of page #{} cannot be restored", page_index + 1),
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::SanitizeDocument { options } => {
            let (_report, updated_bytes) = actor.apply_engine_mutation_for_transaction_manager(
                |doc: &mut dyn PdfEngineDocument| doc.sanitize_document(&options),
            )?;
            let forward = TransactionCommand::SanitizeDocument { options };
            let inverse = TransactionCommand::Irreversible {
                description: "Sanitized document content cannot be restored".to_owned(),
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::Irreversible { description } => Err(TransactionError::Irreversible {
            description: description.clone(),
        }),
        TransactionCommand::RemoveHighlightAnnotation {
            page_index,
            annotation_id,
            ..
        } => {
            let (removed_annot, updated_bytes) = actor
                .apply_engine_mutation_for_transaction_manager(
                    |doc: &mut dyn PdfEngineDocument| {
                        doc.remove_highlight_annotation(page_index, &annotation_id)
                    },
                )?;
            let forward = TransactionCommand::RemoveHighlightAnnotation {
                page_index,
                annotation_id,
                restored_annotation: Some(removed_annot.clone()),
            };
            let inverse = TransactionCommand::AddHighlightAnnotation {
                annotation: removed_annot,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::ReorderPages {
            order,
            allow_signature_invalidation,
        } => {
            let inverse_order = crate::workbench::compute_inverse_permutation(&order)?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.reorder_pages_ext(&order, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::ReorderPages {
                order: order.clone(),
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::ReorderPages {
                order: inverse_order,
                allow_signature_invalidation: true,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::DuplicatePage {
            page_index,
            allow_signature_invalidation,
        } => {
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.duplicate_page_ext(page_index, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::DuplicatePage {
                page_index,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::DeletePages {
                page_indices: vec![page_index + 1],
                allow_signature_invalidation: true,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::DeletePages {
            page_indices,
            allow_signature_invalidation,
        } => {
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.delete_pages_ext(&page_indices, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::DeletePages {
                page_indices,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::RestorePages { previous_bytes } => {
            if previous_bytes.is_empty() {
                return Err(TransactionError::IntegrityMismatch {
                    details: "Cannot execute RestorePages: previous_bytes is empty (fail-closed)"
                        .to_owned(),
                });
            }
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.replace_pdf_bytes(previous_bytes.clone())?;
                Ok(previous_bytes.clone())
            })?;
            let forward = TransactionCommand::RestorePages { previous_bytes };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::InsertPagesFromDocument {
            source_pdf,
            source_page_indices,
            insert_at,
            allow_signature_invalidation,
        } => {
            if source_pdf.is_empty() {
                return Err(TransactionError::IntegrityMismatch {
                    details:
                        "Cannot execute InsertPagesFromDocument: source_pdf is empty (fail-closed)"
                            .to_owned(),
                });
            }
            if source_page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.insert_pages_ext(
                    &source_pdf,
                    &source_page_indices,
                    insert_at,
                    allow_signature_invalidation,
                )
            })?;
            let forward = TransactionCommand::InsertPagesFromDocument {
                source_pdf,
                source_page_indices,
                insert_at,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::MergeDocuments {
            source_pdfs,
            allow_signature_invalidation,
        } => {
            if source_pdfs.is_empty() || source_pdfs.iter().any(|b| b.is_empty()) {
                return Err(TransactionError::IntegrityMismatch {
                    details: "Cannot execute MergeDocuments: source_pdfs is empty or contains empty buffer (fail-closed)".to_owned(),
                });
            }
            let before_bytes = actor.current_pdf_bytes()?;
            let doc_slices: Vec<&[u8]> = source_pdfs.iter().map(|b| b.as_slice()).collect();
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.merge_documents_ext(&doc_slices, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::MergeDocuments {
                source_pdfs,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::SetPageBoxes {
            page_indices,
            boxes,
            previous_boxes,
            allow_signature_invalidation,
        } => {
            if page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let mut captured_prev = previous_boxes;
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                if captured_prev.is_empty() {
                    for &idx in &page_indices {
                        captured_prev.push((idx, doc.get_page_boxes(idx)?));
                    }
                }
                doc.set_page_boxes_ext(&page_indices, &boxes, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::SetPageBoxes {
                page_indices,
                boxes,
                previous_boxes: captured_prev,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::ApplyHardCrop {
            page_indices,
            crop_rect,
            allow_signature_invalidation,
        } => {
            if page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.apply_hard_crop_ext(&page_indices, crop_rect, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::ApplyHardCrop {
                page_indices,
                crop_rect,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::DeskewPages {
            page_indices,
            angle_milli_degrees,
            allow_signature_invalidation,
        } => {
            if page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let before_bytes = actor.current_pdf_bytes()?;
            let angle_deg = angle_milli_degrees as f32 / 1000.0;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.apply_deskew_ext(&page_indices, angle_deg, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::DeskewPages {
                page_indices,
                angle_milli_degrees,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::AutoCropPages {
            page_indices,
            safety_margin_milli_pt,
            previous_boxes,
            allow_signature_invalidation,
        } => {
            if page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let mut captured_prev = previous_boxes;
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                if captured_prev.is_empty() {
                    for &idx in &page_indices {
                        captured_prev.push((idx, doc.get_page_boxes(idx)?));
                    }
                }
                doc.apply_auto_crop_ext(
                    &page_indices,
                    safety_margin_milli_pt,
                    allow_signature_invalidation,
                )
            })?;
            let forward = TransactionCommand::AutoCropPages {
                page_indices,
                safety_margin_milli_pt,
                previous_boxes: captured_prev,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::ApplyBatesNumbering {
            page_indices,
            options,
            allow_signature_invalidation,
        } => {
            if page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.apply_bates_numbering_ext(&page_indices, &options, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::ApplyBatesNumbering {
                page_indices,
                options,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::ApplyHeaderFooter {
            page_indices,
            options,
            doc_title,
            date_str,
            allow_signature_invalidation,
        } => {
            if page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let before_bytes = actor.current_pdf_bytes()?;
            let title_ref = doc_title.as_deref();
            let date_ref = date_str.as_deref();
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.apply_header_footer_ext(
                    &page_indices,
                    &options,
                    title_ref,
                    date_ref,
                    allow_signature_invalidation,
                )
            })?;
            let forward = TransactionCommand::ApplyHeaderFooter {
                page_indices,
                options,
                doc_title,
                date_str,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::SetBookmarks {
            bookmarks,
            previous_bookmarks,
            allow_signature_invalidation,
        } => {
            let mut captured_prev = previous_bookmarks;
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                if captured_prev.is_empty() {
                    captured_prev = doc.get_bookmarks().unwrap_or_default();
                }
                doc.set_bookmarks_ext(&bookmarks, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::SetBookmarks {
                bookmarks,
                previous_bookmarks: captured_prev,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::ApplyStampOrWatermark {
            page_indices,
            options,
            allow_signature_invalidation,
        } => {
            if page_indices.is_empty() {
                return Err(TransactionError::EmptyPageSelection);
            }
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.apply_stamp_or_watermark_ext(
                    &page_indices,
                    &options,
                    allow_signature_invalidation,
                )
            })?;
            let forward = TransactionCommand::ApplyStampOrWatermark {
                page_indices,
                options,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
        TransactionCommand::OptimizeDocument {
            options,
            allow_signature_invalidation,
        } => {
            let before_bytes = actor.current_pdf_bytes()?;
            let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                doc.optimize_document_ext(&options, allow_signature_invalidation)
            })?;
            let forward = TransactionCommand::OptimizeDocument {
                options,
                allow_signature_invalidation,
            };
            let inverse = TransactionCommand::RestorePages {
                previous_bytes: before_bytes,
            };
            Ok((forward, inverse, updated_bytes))
        }
    }
}
