//! Persistent transaction journal representation (`ARCHITECTURE.md` §§11–12, `UNDO-001`, `ROADMAP.md` M0.4).
//!
//! # Threat Model & Integrity Scope (ROLE-09 Security, ROLE-10 Privacy)
//!
//! - **Integrity & Crash Recovery:** The journal employs a deterministic unkeyed SHA-256
//!   digest chain covering the schema version, document identity, base PDF digest, revision
//!   counters, individual transaction records, and serialized commands. This provides robust
//!   integrity verification against accidental file corruption, bitrot, power-loss/interrupted
//!   writes, and schema mismatches during crash recovery.
//! - **Threat Model Boundary:** Unkeyed SHA-256 is a cryptographic hash, NOT an HMAC or digital
//!   signature. It does not use a secret key and therefore does not provide tamper-resistance
//!   against an active, privileged attacker with write access to the local storage, who could
//!   modify the journal contents and recompute valid checksums.
//! - **Confidentiality & Access Control:** Journal confidentiality rests strictly on operating
//!   system security perimeters: App Sandbox (macOS), private application directory / Storage
//!   Access Framework (Android), POSIX permissions, and OS-level full-disk encryption (FileVault /
//!   LUKS / Android FBE).
//! - **Key Management & Cryptography:** As an offline-first tool without central accounts,
//!   Vocument avoids proprietary key management. For document-level cryptographic access control,
//!   standard PDF encryption (ISO 32000 AES-256 with user/owner passwords) or platform key stores
//!   (macOS Keychain, Android KeyStore) are used.
//! - **Privacy & Storage Minimization:** Persistent journals strip raw binary document dumps
//!   from inverse commands prior to serialization, preventing unneeded plaintext duplication
//!   on disk.

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
