//! `DocumentSession` and `SessionRegistry` (`ARCHITECTURE.md` §6, `ROADMAP.md` M0.1).
//!
//! Each opened document receives an application-owned `DocumentSession` with stable
//! `SessionId`, `DocumentId`, `FileIdentity`, revision token, and `PdfEngineDocument` handle.

use crate::file_broker::{BrokeredFileRead, FileAccessOrigin, FileBroker, FileBrokerError};
use crate::identity::{DocumentId, FileIdentity, FileIdentityComparison, SessionId};
use crate::pdf_engine::{PdfDocumentSummary, PdfEngine, PdfEngineDocument, PdfEngineError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionError {
    #[error(transparent)]
    FileBroker(#[from] FileBrokerError),
    #[error(transparent)]
    PdfEngine(#[from] PdfEngineError),
    #[error("unknown session ID '{session_id}'")]
    SessionNotFound { session_id: String },
    #[error("session registry lock poisoned")]
    RegistryPoisoned,
}

/// Immutable snapshot of an active `DocumentSession` exposed to native UI across UniFFI (`ARCHITECTURE.md` §§5–6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSessionSnapshot {
    pub session_id: SessionId,
    pub document_id: DocumentId,
    pub revision: u64,
    pub is_dirty: bool,
    pub access_origin: FileAccessOrigin,
    pub file_identity: FileIdentity,
    pub pdf_summary: PdfDocumentSummary,
}

/// Application-owned live state for one opened document (`ARCHITECTURE.md` §6).
#[derive(Debug)]
pub struct DocumentSession {
    session_id: SessionId,
    document_id: DocumentId,
    revision: u64,
    is_dirty: bool,
    access_origin: FileAccessOrigin,
    file_identity: FileIdentity,
    engine_document: Box<dyn PdfEngineDocument>,
}

impl DocumentSession {
    pub fn snapshot(&self) -> DocumentSessionSnapshot {
        DocumentSessionSnapshot {
            session_id: self.session_id.clone(),
            document_id: self.document_id.clone(),
            revision: self.revision,
            is_dirty: self.is_dirty,
            access_origin: self.access_origin,
            file_identity: self.file_identity.clone(),
            pdf_summary: self.engine_document.summary().clone(),
        }
    }

    pub fn compare_external_identity(&self, observed: &FileIdentity) -> FileIdentityComparison {
        self.file_identity.compare_with(observed)
    }
}

/// Thread-safe registry owning active `DocumentSession` instances inside the Rust core.
#[derive(Debug)]
pub struct SessionRegistry {
    pdf_engine: Box<dyn PdfEngine>,
    next_session_number: AtomicU64,
    sessions: Mutex<BTreeMap<String, DocumentSession>>,
}

impl SessionRegistry {
    pub fn new(pdf_engine: Box<dyn PdfEngine>) -> Self {
        Self {
            pdf_engine,
            next_session_number: AtomicU64::new(1),
            sessions: Mutex::new(BTreeMap::new()),
        }
    }

    /// Opens a PDF through a `FileBroker` source token and registers a new `DocumentSession`.
    pub fn open_via_broker(
        &self,
        broker: &dyn FileBroker,
        source_token: &str,
    ) -> Result<DocumentSessionSnapshot, SessionError> {
        let brokered = broker.read_file(source_token)?;
        self.open_brokered_file(brokered)
    }

    /// Opens a PDF from a platform-provided `BrokeredFileRead` (e.g. macOS security-scoped file or Android SAF descriptor).
    pub fn open_brokered_file(
        &self,
        brokered: BrokeredFileRead,
    ) -> Result<DocumentSessionSnapshot, SessionError> {
        if brokered.bytes.is_empty() {
            return Err(SessionError::FileBroker(FileBrokerError::EmptyPayload {
                source_token: brokered.source_token,
            }));
        }

        let file_identity = brokered.compute_identity();
        let document_id = file_identity.stable_document_id();
        let engine_document = self.pdf_engine.open_pdf_bytes(&brokered.bytes)?;

        let counter = self.next_session_number.fetch_add(1, Ordering::Relaxed);
        let session_id = SessionId::new(counter, &document_id);

        let session = DocumentSession {
            session_id: session_id.clone(),
            document_id,
            revision: 1,
            is_dirty: false,
            access_origin: brokered.access_origin,
            file_identity,
            engine_document,
        };

        let snapshot = session.snapshot();
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        guard.insert(session_id.as_str().to_owned(), session);
        Ok(snapshot)
    }

    /// Retrieves an immutable snapshot of an existing `DocumentSession`.
    pub fn session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<DocumentSessionSnapshot, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        Ok(session.snapshot())
    }

    /// Closes an active `DocumentSession` and releases its underlying engine resources.
    pub fn close_session(&self, session_id: &str) -> Result<(), SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        if guard.remove(session_id).is_none() {
            return Err(SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            });
        }
        Ok(())
    }

    /// Returns the number of currently active sessions.
    pub fn active_session_count(&self) -> Result<usize, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        Ok(guard.len())
    }
}
