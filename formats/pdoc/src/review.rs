//! Review metadata, comments, and tracked changes container (`review.json`, `ADR-0020`, `TRACK-001`–`006`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Review comment thread anchored to a character range or block in the document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdocFlowComment {
    pub comment_id: String,
    pub author: String,
    pub timestamp: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_block_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_char_start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_char_end: Option<usize>,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replies: Vec<PdocFlowCommentReply>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Reply within a comment thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdocFlowCommentReply {
    pub reply_id: String,
    pub author: String,
    pub timestamp: String,
    pub content: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Type of revision in tracked changes (`TRACK-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PdocChangeType {
    Insert,
    Delete,
    FormatChange,
}

/// Tracked revision record attributing changes to authors (`TRACK-001`–`003`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdocTrackedChange {
    pub change_id: String,
    pub change_type: PdocChangeType,
    pub author: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_block_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Root review container structure stored as `review.json`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PdocReviewData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<PdocFlowComment>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracked_changes: Vec<PdocTrackedChange>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl PdocReviewData {
    pub fn is_empty(&self) -> bool {
        self.comments.is_empty() && self.tracked_changes.is_empty() && self.extra.is_empty()
    }
}
