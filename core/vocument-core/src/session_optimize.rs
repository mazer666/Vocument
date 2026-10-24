//! SessionRegistry extensions for M3.5 Optimization, Image Extraction, and Preflight (`ROADMAP.md` M3.5).

use crate::pdf_engine::{
    ExtractedImageData, ExtractedImageInfo, OptimizationOptions, PdfxPreflightReport,
    PrintPreflightReport,
};
use crate::session::{SessionError, SessionRegistry};
use crate::transactions::{TransactionCommand, TransactionRecord};
use crate::workbench::PageWorkbenchOptions;

impl SessionRegistry {
    /// Optimizes and compresses the session document with transaction support (`ROADMAP.md` M3.5).
    pub fn optimize_document_with_options(
        &self,
        session_id: &str,
        options: OptimizationOptions,
        workbench_options: PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !workbench_options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::OptimizeDocument {
                options,
                allow_signature_invalidation: workbench_options.allow_signature_invalidation,
            },
        )
    }

    /// Discovers all embedded raster images in the document (`M3.5`).
    pub fn get_document_images(
        &self,
        session_id: &str,
    ) -> Result<Vec<ExtractedImageInfo>, SessionError> {
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
        Ok(actor.get_document_images()?)
    }

    /// Extracts raw/PNG image bytes for a specified page and image index (`M3.5`).
    pub fn extract_image_bytes(
        &self,
        session_id: &str,
        page_index: u32,
        image_index: u32,
    ) -> Result<ExtractedImageData, SessionError> {
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
        Ok(actor.extract_image_bytes(page_index, image_index)?)
    }

    /// Performs preflight verification for PDF/X (ISO 15930) compliance (`M3.5`).
    pub fn preflight_pdfx(&self, session_id: &str) -> Result<PdfxPreflightReport, SessionError> {
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
        Ok(actor.preflight_pdfx()?)
    }

    /// Performs preflight verification for print readiness (`PRINT-003`, `PRINT-004`).
    pub fn preflight_print(&self, session_id: &str) -> Result<PrintPreflightReport, SessionError> {
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
        Ok(actor.preflight_print()?)
    }
}
