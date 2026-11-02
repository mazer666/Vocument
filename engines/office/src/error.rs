//! Error type of the DOCX/ODT adapters (`OFFICE-001`–`OFFICE-005`, `ADR-0020`).

use vocument_flow::FlowError;

/// Failure to import or export an Office document.
///
/// Every variant is a *refusal with a reason*: the adapters never fall back to a partial,
/// silently degraded result (`OFFICE-004`).
#[derive(Debug, thiserror::Error)]
pub enum OfficeError {
    /// The bytes are not a ZIP package.
    #[error("not a ZIP-based Office package: {details}")]
    NotAZipPackage { details: String },

    /// The file is an OLE compound document: a legacy `.doc` or a password-protected package.
    #[error(
        "the file is an OLE compound document (legacy .doc/.xls format or a password-protected \
         Office file); neither can be opened"
    )]
    PasswordProtectedOrLegacy,

    /// A ZIP entry is encrypted.
    #[error("package entry '{name}' is encrypted")]
    EncryptedEntry { name: String },

    /// A resource limit protecting against decompression bombs and oversized input was hit.
    #[error("resource limit exceeded: {what} (limit {limit})")]
    ResourceLimitExceeded { what: String, limit: u64 },

    /// A ZIP entry name could escape the package (path traversal) or is otherwise malformed.
    #[error("unsafe package entry name '{name}': {reason}")]
    UnsafeEntryName { name: String, reason: String },

    /// A part the format requires is absent.
    #[error("required package part '{name}' is missing")]
    MissingPart { name: String },

    /// An XML part is not well-formed or uses a prohibited construct (DTD, entity declaration).
    #[error("malformed XML in '{part}': {details}")]
    MalformedXml { part: String, details: String },

    /// The package is valid but does not contain a document this adapter understands.
    #[error("unsupported document: {details}")]
    UnsupportedDocument { details: String },

    /// The imported content violates the `FlowDocument` invariants.
    #[error("imported content violates the document model: {0}")]
    InvalidModel(#[from] FlowError),

    /// A ZIP-level failure that is not covered by a more specific variant.
    #[error("ZIP error: {0}")]
    Zip(String),

    /// An I/O failure while writing a package.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<zip::result::ZipError> for OfficeError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error.to_string())
    }
}
