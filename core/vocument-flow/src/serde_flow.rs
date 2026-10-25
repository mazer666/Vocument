//! Canonical JSON serialization and deserialization for FlowDocument (`ADR-0020`, `SOURCE-001`, `SOURCE-002`).

use crate::error::FlowError;
use crate::model::FlowDocument;
use crate::schema::validate_document;

/// Serializes a `FlowDocument` to a deterministic, compact UTF-8 JSON string.
pub fn to_json_canonical(doc: &FlowDocument) -> Result<String, FlowError> {
    validate_document(doc)?;
    serde_json::to_string(doc).map_err(|err| FlowError::SerializationError {
        details: err.to_string(),
    })
}

/// Serializes a `FlowDocument` to a pretty-printed UTF-8 JSON string for inspection and debugging.
pub fn to_json_pretty(doc: &FlowDocument) -> Result<String, FlowError> {
    validate_document(doc)?;
    serde_json::to_string_pretty(doc).map_err(|err| FlowError::SerializationError {
        details: err.to_string(),
    })
}

/// Deserializes and validates a `FlowDocument` from a JSON string.
pub fn from_json_str(json: &str) -> Result<FlowDocument, FlowError> {
    let doc: FlowDocument =
        serde_json::from_str(json).map_err(|err| FlowError::DeserializationError {
            details: err.to_string(),
        })?;
    validate_document(&doc)?;
    Ok(doc)
}

/// Deserializes and validates a `FlowDocument` from raw UTF-8 bytes.
pub fn from_json_bytes(bytes: &[u8]) -> Result<FlowDocument, FlowError> {
    let doc: FlowDocument =
        serde_json::from_slice(bytes).map_err(|err| FlowError::DeserializationError {
            details: err.to_string(),
        })?;
    validate_document(&doc)?;
    Ok(doc)
}

impl FlowDocument {
    /// Serializes this document to compact canonical JSON.
    pub fn to_json(&self) -> Result<String, FlowError> {
        to_json_canonical(self)
    }

    /// Serializes this document to pretty-printed JSON.
    pub fn to_json_pretty(&self) -> Result<String, FlowError> {
        to_json_pretty(self)
    }

    /// Parses and validates a `FlowDocument` from JSON.
    pub fn from_json(json: &str) -> Result<Self, FlowError> {
        from_json_str(json)
    }
}
