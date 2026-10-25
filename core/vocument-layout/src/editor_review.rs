//! Reviewing system integration for FlowEditorState (`TRACK-001`–`006`).
//!
//! Provides tracked changes recording, individual/bulk accept/reject,
//! thread comments with resilient text anchoring, replies, and resolution.

use vocument_flow::review::{ChangeType, FlowComment, FlowCommentReply};

use crate::editor::FlowEditorState;
use crate::editor_spans::{block_id, for_each_span_in_block_mut, retain_spans_in_block, uuid_fast};

impl FlowEditorState {
    /// Enables change tracking under the given author attribution.
    pub fn enable_tracking(&mut self, author: impl Into<String>) {
        self.tracking_enabled = true;
        self.active_author = author.into();
    }

    /// Disables change tracking for subsequent edits.
    pub fn disable_tracking(&mut self) {
        self.tracking_enabled = false;
    }

    /// Checks whether change tracking is currently active.
    pub fn is_tracking(&self) -> bool {
        self.tracking_enabled
    }

    /// Adds a margin comment anchored to the active block and optional character range.
    pub fn add_comment(
        &mut self,
        anchor_range: Option<(usize, usize)>,
        content: impl Into<String>,
    ) -> String {
        self.push_undo_snapshot();
        let comment_id = format!("comm_{}", uuid_fast());
        let timestamp = format!(
            "unix:{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );

        let block_id = self.active_block_id.clone();
        let anchor_text = if let Some((start, end)) = anchor_range {
            self.extract_text_range(&block_id, start, end)
        } else {
            None
        };

        let comment = FlowComment::new(
            &comment_id,
            &self.active_author,
            &timestamp,
            content,
            Some(block_id),
            anchor_range,
            anchor_text,
        );

        self.document.review.comments.push(comment);
        comment_id
    }

    /// Appends a reply to an existing comment thread.
    pub fn reply_to_comment(
        &mut self,
        comment_id: &str,
        content: impl Into<String>,
    ) -> Option<String> {
        self.push_undo_snapshot();
        let reply_id = format!("rep_{}", uuid_fast());
        let timestamp = format!(
            "unix:{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );

        let reply = FlowCommentReply::new(&reply_id, &self.active_author, &timestamp, content);

        if let Some(comment) = self
            .document
            .review
            .comments
            .iter_mut()
            .find(|c| c.comment_id == comment_id)
        {
            comment.replies.push(reply);
            Some(reply_id)
        } else {
            None
        }
    }

    /// Sets the resolved status of a comment thread.
    pub fn resolve_comment(&mut self, comment_id: &str, resolved: bool) -> bool {
        self.push_undo_snapshot();
        if let Some(comment) = self
            .document
            .review
            .comments
            .iter_mut()
            .find(|c| c.comment_id == comment_id)
        {
            comment.resolved = resolved;
            true
        } else {
            false
        }
    }

    /// Deletes a comment thread completely.
    pub fn delete_comment(&mut self, comment_id: &str) -> bool {
        self.push_undo_snapshot();
        let initial_len = self.document.review.comments.len();
        self.document
            .review
            .comments
            .retain(|c| c.comment_id != comment_id);
        self.document.review.comments.len() < initial_len
    }

    /// Accepts an individual tracked change by ID (`TRACK-003`).
    pub fn accept_change(&mut self, change_id: &str) -> bool {
        let change_idx = match self
            .document
            .review
            .tracked_changes
            .iter()
            .position(|c| c.change_id == change_id)
        {
            Some(idx) => idx,
            None => return false,
        };

        self.push_undo_snapshot();
        let change = self.document.review.tracked_changes.remove(change_idx);

        match change.change_type {
            ChangeType::Insert | ChangeType::FormatChange => {
                // Clear tracked change ID; text and format remain permanently in document
                self.clear_tracked_change_id_on_spans(change_id);
            }
            ChangeType::Delete => {
                // Physically remove the marked deleted spans
                self.remove_spans_by_tracked_change_id(change_id);
            }
        }

        self.recompute_full_layout();
        true
    }

    /// Rejects an individual tracked change by ID (`TRACK-003`).
    pub fn reject_change(&mut self, change_id: &str) -> bool {
        let change_idx = match self
            .document
            .review
            .tracked_changes
            .iter()
            .position(|c| c.change_id == change_id)
        {
            Some(idx) => idx,
            None => return false,
        };

        self.push_undo_snapshot();
        let change = self.document.review.tracked_changes.remove(change_idx);

        match change.change_type {
            ChangeType::Insert => {
                // Remove the inserted spans
                self.remove_spans_by_tracked_change_id(change_id);
            }
            ChangeType::Delete => {
                // Restore span: clear strikethrough and clear tracked change ID
                self.restore_deleted_spans_by_tracked_change_id(change_id);
            }
            ChangeType::FormatChange => {
                self.revert_format_change_by_tracked_change_id(change_id);
            }
        }

        self.recompute_full_layout();
        true
    }

    /// Accepts all pending tracked changes across the document.
    pub fn accept_all_changes(&mut self) {
        if self.document.review.tracked_changes.is_empty() {
            return;
        }
        self.push_undo_snapshot();
        let changes = std::mem::take(&mut self.document.review.tracked_changes);
        for change in changes {
            match change.change_type {
                ChangeType::Insert | ChangeType::FormatChange => {
                    self.clear_tracked_change_id_on_spans(&change.change_id);
                }
                ChangeType::Delete => {
                    self.remove_spans_by_tracked_change_id(&change.change_id);
                }
            }
        }
        self.recompute_full_layout();
    }

    /// Rejects all pending tracked changes across the document.
    pub fn reject_all_changes(&mut self) {
        if self.document.review.tracked_changes.is_empty() {
            return;
        }
        self.push_undo_snapshot();
        let changes = std::mem::take(&mut self.document.review.tracked_changes);
        for change in changes {
            match change.change_type {
                ChangeType::Insert => {
                    self.remove_spans_by_tracked_change_id(&change.change_id);
                }
                ChangeType::Delete => {
                    self.restore_deleted_spans_by_tracked_change_id(&change.change_id);
                }
                ChangeType::FormatChange => {
                    self.revert_format_change_by_tracked_change_id(&change.change_id);
                }
            }
        }
        self.recompute_full_layout();
    }

    /// Resilient anchor updater: shifts comment anchors when characters are inserted before or within them.
    pub fn adjust_comment_anchors_on_insert(
        &mut self,
        block_id: &str,
        offset: usize,
        count: usize,
    ) {
        for comment in &mut self.document.review.comments {
            if comment.anchor_block_id.as_deref() == Some(block_id) {
                if let (Some(start), Some(end)) =
                    (comment.anchor_char_start, comment.anchor_char_end)
                {
                    if offset <= start {
                        comment.anchor_char_start = Some(start + count);
                        comment.anchor_char_end = Some(end + count);
                    } else if offset < end {
                        comment.anchor_char_end = Some(end + count);
                    }
                }
            }
        }
    }

    /// Resilient anchor updater: shifts or shrinks comment anchors when characters are deleted.
    pub fn adjust_comment_anchors_on_delete(
        &mut self,
        block_id: &str,
        offset: usize,
        count: usize,
    ) {
        let del_end = offset + count;
        for comment in &mut self.document.review.comments {
            if comment.anchor_block_id.as_deref() == Some(block_id) {
                if let (Some(start), Some(end)) =
                    (comment.anchor_char_start, comment.anchor_char_end)
                {
                    if del_end <= start {
                        comment.anchor_char_start = Some(start.saturating_sub(count));
                        comment.anchor_char_end = Some(end.saturating_sub(count));
                    } else if offset >= end {
                        // Deletion is after anchor, unaffected
                    } else {
                        // Deletion intersects anchor
                        let new_start = start.min(offset);
                        let removed_in_anchor = del_end.min(end).saturating_sub(offset.max(start));
                        let new_end = end.saturating_sub(removed_in_anchor).max(new_start);
                        comment.anchor_char_start = Some(new_start);
                        comment.anchor_char_end = Some(new_end);
                    }
                }
            }
        }
    }

    fn extract_text_range(
        &self,
        target_block_id: &str,
        start_offset: usize,
        end_offset: usize,
    ) -> Option<String> {
        let min_off = start_offset.min(end_offset);
        let max_off = start_offset.max(end_offset);
        if min_off == max_off {
            return None;
        }

        let full_text = self
            .document
            .sections
            .iter()
            .flat_map(|s| &s.blocks)
            .find_map(|b| {
                if block_id(b) == target_block_id {
                    match b {
                        vocument_flow::model::Block::Paragraph(p) => {
                            Some(p.spans.iter().map(|s| s.text.as_str()).collect::<String>())
                        }
                        vocument_flow::model::Block::Heading(h) => {
                            Some(h.spans.iter().map(|s| s.text.as_str()).collect::<String>())
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            })?;

        let char_count = full_text.chars().count();
        if min_off >= char_count {
            return None;
        }

        let snippet = full_text
            .chars()
            .skip(min_off)
            .take(max_off.saturating_sub(min_off))
            .collect::<String>();
        Some(snippet)
    }

    fn clear_tracked_change_id_on_spans(&mut self, change_id: &str) {
        for section in &mut self.document.sections {
            for block in &mut section.blocks {
                for_each_span_in_block_mut(block, &mut |span| {
                    if span.tracked_change_id.as_deref() == Some(change_id) {
                        span.original_style = None;
                        span.tracked_change_id = None;
                    }
                });
            }
        }
    }

    fn revert_format_change_by_tracked_change_id(&mut self, change_id: &str) {
        for section in &mut self.document.sections {
            for block in &mut section.blocks {
                for_each_span_in_block_mut(block, &mut |span| {
                    if span.tracked_change_id.as_deref() == Some(change_id) {
                        if let Some(orig) = span.original_style.take() {
                            span.style = orig;
                        }
                        span.tracked_change_id = None;
                    }
                });
            }
        }
    }

    fn remove_spans_by_tracked_change_id(&mut self, change_id: &str) {
        for section in &mut self.document.sections {
            for block in &mut section.blocks {
                retain_spans_in_block(block, &mut |s| {
                    s.tracked_change_id.as_deref() != Some(change_id)
                });
            }
        }
    }

    fn restore_deleted_spans_by_tracked_change_id(&mut self, change_id: &str) {
        for section in &mut self.document.sections {
            for block in &mut section.blocks {
                for_each_span_in_block_mut(block, &mut |span| {
                    if span.tracked_change_id.as_deref() == Some(change_id) {
                        span.style.strikethrough = false;
                        span.tracked_change_id = None;
                    }
                });
            }
        }
    }
}
