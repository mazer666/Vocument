//! Dublin Core metadata and document provenance (`ADR-0020`, `AUTHOR-001`, `SOURCE-001`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::FlowError;

/// Document author provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affiliation: Option<String>,
}

impl Author {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
            affiliation: None,
        }
    }

    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    pub fn with_affiliation(mut self, affiliation: impl Into<String>) -> Self {
        self.affiliation = Some(affiliation.into());
        self
    }
}

/// Dublin Core and authored document metadata (`SOURCE-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Unique document identifier (UUID string).
    pub id: String,

    /// Primary document title.
    pub title: String,

    /// Optional document subtitle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,

    /// List of contributing authors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<Author>,

    /// Abstract or brief description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Primary document language code (BCP 47, e.g. "de-DE", "en-US").
    #[serde(default = "default_language")]
    pub language: String,

    /// Creation timestamp in ISO 8601 UTC.
    pub created_at_utc: String,

    /// Last modification timestamp in ISO 8601 UTC.
    pub modified_at_utc: String,

    /// Monotonic revision sequence number.
    #[serde(default)]
    pub revision: u64,

    /// Subject categorization keywords / tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,

    /// Arbitrary custom key-value metadata attributes (Dublin Core extensions).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_attributes: BTreeMap<String, String>,

    /// Unknown or forward-compatible JSON attributes preserved across roundtrips.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn default_language() -> String {
    "en".to_string()
}

impl DocumentMetadata {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        let now = "1970-01-01T00:00:00Z".to_string();
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            authors: Vec::new(),
            description: None,
            language: default_language(),
            created_at_utc: now.clone(),
            modified_at_utc: now,
            revision: 1,
            keywords: Vec::new(),
            custom_attributes: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }

    /// Validates metadata integrity invariants.
    pub fn validate(&self) -> Result<(), FlowError> {
        if self.id.trim().is_empty() {
            return Err(FlowError::InvalidMetadata {
                field: "id".to_string(),
                details: "document id cannot be empty".to_string(),
            });
        }
        if self.title.trim().is_empty() {
            return Err(FlowError::InvalidMetadata {
                field: "title".to_string(),
                details: "document title cannot be empty".to_string(),
            });
        }
        if self.language.trim().is_empty() {
            return Err(FlowError::InvalidMetadata {
                field: "language".to_string(),
                details: "document language tag cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}
