//! SessionRegistry extensions for M3.4 Bates Numbering, Headers/Footers, Bookmarks, and Stamps/Watermarks (`ROADMAP.md` M3.4).

use crate::session::{SessionError, SessionRegistry};
use crate::transactions::{TransactionCommand, TransactionError, TransactionRecord};
use crate::workbench::PageWorkbenchOptions;
use crate::workbench_stamps::{
    BatesNumberingOptions, BookmarkItem, HeaderFooterOptions, StampOrWatermarkOptions,
};

impl SessionRegistry {
    /// Applies Bates numbering to selected pages with preflight checks (`ROADMAP.md` M3.4).
    pub fn apply_bates_numbering_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        options: BatesNumberingOptions,
        workbench_options: PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        if page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
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
            TransactionCommand::ApplyBatesNumbering {
                page_indices,
                options,
                allow_signature_invalidation: workbench_options.allow_signature_invalidation,
            },
        )
    }

    /// Applies multi-zone headers and footers with dynamic macros (`ROADMAP.md` M3.4).
    pub fn apply_header_footer_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        options: HeaderFooterOptions,
        doc_title: Option<String>,
        date_str: Option<String>,
        workbench_options: PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        if page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
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
            TransactionCommand::ApplyHeaderFooter {
                page_indices,
                options,
                doc_title,
                date_str,
                allow_signature_invalidation: workbench_options.allow_signature_invalidation,
            },
        )
    }

    /// Retrieves document bookmark (outline) hierarchy (`ROADMAP.md` M3.4).
    pub fn get_bookmarks(&self, session_id: &str) -> Result<Vec<BookmarkItem>, SessionError> {
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
        Ok(actor.get_bookmarks()?)
    }

    /// Replaces document bookmark (outline) hierarchy with preflight checks (`ROADMAP.md` M3.4).
    pub fn set_bookmarks_with_options(
        &self,
        session_id: &str,
        bookmarks: Vec<BookmarkItem>,
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
            TransactionCommand::SetBookmarks {
                bookmarks,
                previous_bookmarks: Vec::new(),
                allow_signature_invalidation: workbench_options.allow_signature_invalidation,
            },
        )
    }

    /// Applies rubber stamp or watermark overlay to selected pages (`WATER-001`–`003`, `ROADMAP.md` M3.4).
    pub fn apply_stamp_or_watermark_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        options: StampOrWatermarkOptions,
        workbench_options: PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        if page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
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
            TransactionCommand::ApplyStampOrWatermark {
                page_indices,
                options,
                allow_signature_invalidation: workbench_options.allow_signature_invalidation,
            },
        )
    }
}
