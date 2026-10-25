//! Reviewing metadata, margin comments, replies, and tracked changes (`review.json`, `ADR-0020`, `TRACK-001`–`006`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Review comment thread anchored to a character range or block in the document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowComment {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_text: Option<String>,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replies: Vec<FlowCommentReply>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl FlowComment {
    pub fn new(
        comment_id: impl Into<String>,
        author: impl Into<String>,
        timestamp: impl Into<String>,
        content: impl Into<String>,
        anchor_block_id: Option<String>,
        anchor_range: Option<(usize, usize)>,
        anchor_text: Option<String>,
    ) -> Self {
        let (start, end) = match anchor_range {
            Some((s, e)) => (Some(s), Some(e)),
            None => (None, None),
        };
        Self {
            comment_id: comment_id.into(),
            author: author.into(),
            timestamp: timestamp.into(),
            content: content.into(),
            anchor_block_id,
            anchor_char_start: start,
            anchor_char_end: end,
            anchor_text,
            resolved: false,
            replies: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

/// Reply within a comment thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowCommentReply {
    pub reply_id: String,
    pub author: String,
    pub timestamp: String,
    pub content: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl FlowCommentReply {
    pub fn new(
        reply_id: impl Into<String>,
        author: impl Into<String>,
        timestamp: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            reply_id: reply_id.into(),
            author: author.into(),
            timestamp: timestamp.into(),
            content: content.into(),
            extra: BTreeMap::new(),
        }
    }
}

/// Type of revision in tracked changes (`TRACK-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Insert,
    Delete,
    FormatChange,
}

/// Tracked revision record attributing changes to authors (`TRACK-001`–`003`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackedChange {
    pub change_id: String,
    pub change_type: ChangeType,
    pub author: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_block_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_snippet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl TrackedChange {
    pub fn new(
        change_id: impl Into<String>,
        change_type: ChangeType,
        author: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            change_id: change_id.into(),
            change_type,
            author: author.into(),
            timestamp: timestamp.into(),
            target_block_id: None,
            text_snippet: None,
            details: None,
            extra: BTreeMap::new(),
        }
    }
}

/// Root review container structure stored as `review.json`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FlowReviewData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<FlowComment>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracked_changes: Vec<TrackedChange>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl FlowReviewData {
    pub fn is_empty(&self) -> bool {
        self.comments.is_empty() && self.tracked_changes.is_empty() && self.extra.is_empty()
    }
}
