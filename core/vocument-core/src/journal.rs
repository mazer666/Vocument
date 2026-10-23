//! Persistent transaction journal representation (`ARCHITECTURE.md` §§11–12, `UNDO-001`, `ROADMAP.md` M0.4).

use crate::identity::compute_sha256_hex as sha256_hex;
use crate::transactions::{
    TRANSACTION_JOURNAL_SCHEMA_VERSION, TransactionError, TransactionRecord,
};
use serde::{Deserialize, Serialize};

/// Persistent transaction journal representation (`ARCHITECTURE.md` §§11–12, `UNDO-001`, `ROADMAP.md` M0.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistentTransactionJournal {
    pub schema_version: u32,
    pub document_id: String,
    pub base_pdf_sha256: String,
    pub current_revision: u64,
    pub saved_revision: u64,
    pub committed_records: Vec<TransactionRecord>,
    pub redo_records: Vec<TransactionRecord>,
    pub journal_checksum_sha256: String,
}

impl PersistentTransactionJournal {
    pub fn compute_journal_checksum(
        schema_version: u32,
        document_id: &str,
        base_pdf_sha256: &str,
        current_revision: u64,
        saved_revision: u64,
        committed_records: &[TransactionRecord],
        redo_records: &[TransactionRecord],
    ) -> String {
        let mut material = format!(
            "v={schema_version}|doc={document_id}|base={base_pdf_sha256}|cur={current_revision}|saved={saved_revision}|committed="
        );
        for rec in committed_records {
            material.push_str(&rec.record_checksum_sha256);
            material.push(',');
        }
        material.push_str("|redo=");
        for rec in redo_records {
            material.push_str(&rec.record_checksum_sha256);
            material.push(',');
        }
        sha256_hex(material.as_bytes())
    }

    /// Verifies schema version, every individual record checksum, and the overall journal checksum (`ARCHITECTURE.md` §12).
    pub fn verify_integrity(&self) -> Result<(), TransactionError> {
        if self.schema_version != TRANSACTION_JOURNAL_SCHEMA_VERSION {
            return Err(TransactionError::IntegrityMismatch {
                details: format!(
                    "unsupported journal schema_version {} (expected {})",
                    self.schema_version, TRANSACTION_JOURNAL_SCHEMA_VERSION
                ),
            });
        }
        for rec in self
            .committed_records
            .iter()
            .chain(self.redo_records.iter())
        {
            if !rec.verify_checksum() {
                return Err(TransactionError::IntegrityMismatch {
                    details: format!(
                        "record checksum mismatch on transaction '{}'",
                        rec.transaction_id
                    ),
                });
            }
        }
        let expected = Self::compute_journal_checksum(
            self.schema_version,
            &self.document_id,
            &self.base_pdf_sha256,
            self.current_revision,
            self.saved_revision,
            &self.committed_records,
            &self.redo_records,
        );
        if expected != self.journal_checksum_sha256 {
            return Err(TransactionError::IntegrityMismatch {
                details: "overall journal_checksum_sha256 mismatch".to_owned(),
            });
        }
        Ok(())
    }

    /// Serializes this persistent transaction journal to pretty-printed JSON (`ARCHITECTURE.md` §12).
    pub fn to_json_pretty(&self) -> Result<String, TransactionError> {
        serde_json::to_string_pretty(self).map_err(|err| TransactionError::SerializationFailure {
            details: err.to_string(),
        })
    }

    /// Parses and integrity-verifies a `PersistentTransactionJournal` from JSON.
    pub fn from_json_verified(json: &str) -> Result<Self, TransactionError> {
        let parsed: Self =
            serde_json::from_str(json).map_err(|err| TransactionError::SerializationFailure {
                details: err.to_string(),
            })?;
        parsed.verify_integrity()?;
        Ok(parsed)
    }
}
