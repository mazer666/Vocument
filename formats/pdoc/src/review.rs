//! Review metadata, comments, and tracked changes container (`review.json`, `ADR-0020`, `TRACK-001`–`006`).

pub use vocument_flow::review::{
    ChangeType as PdocChangeType, FlowComment as PdocFlowComment,
    FlowCommentReply as PdocFlowCommentReply, FlowReviewData as PdocReviewData,
    TrackedChange as PdocTrackedChange,
};
