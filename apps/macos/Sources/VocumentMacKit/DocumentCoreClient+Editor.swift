import Foundation
import VocumentFFI

/// Swift extensions for M4.3 Native Flow Document Editing (`ADR-0020`, `UI-009`).
extension DocumentCoreClient {
    /// Creates a new in-memory `.pdoc` flow editor session.
    public func createEditorSession(title: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreCreateEditorSession(title: title)
    }

    /// Opens an existing `.pdoc` file package into the native editor.
    public func openPdocEditorSession(at path: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreOpenPdocEditorSession(filePath: path)
    }

    /// Closes an active editor session.
    public func closeEditorSession(editorId: String) throws {
        try documentCoreCloseEditorSession(editorId: editorId)
    }

    /// Inserts text at the cursor with sub-16ms layout reflow.
    public func editorInsertText(editorId: String, text: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorInsertText(editorId: editorId, text: text)
    }

    /// Inserts a paragraph break (Enter key).
    public func editorInsertParagraphBreak(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorInsertParagraphBreak(editorId: editorId)
    }

    /// Deletes backward from cursor (Backspace).
    public func editorDeleteBackward(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorDeleteBackward(editorId: editorId)
    }

    /// Deletes forward from cursor (Forward Delete).
    public func editorDeleteForward(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorDeleteForward(editorId: editorId)
    }

    /// Sets cursor position within a block.
    public func editorSetCursor(editorId: String, blockId: String, offset: UInt32) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorSetCursor(editorId: editorId, blockId: blockId, offset: offset)
    }

    /// Sets selection range between blocks.
    public func editorSetSelection(
        editorId: String,
        anchorBlockId: String,
        anchorOffset: UInt32,
        focusBlockId: String,
        focusOffset: UInt32
    ) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorSetSelection(
            editorId: editorId,
            anchorBlockId: anchorBlockId,
            anchorOffset: anchorOffset,
            focusBlockId: focusBlockId,
            focusOffset: focusOffset
        )
    }

    /// Clears active selection.
    public func editorClearSelection(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorClearSelection(editorId: editorId)
    }

    /// Coordinate hit-test at (x, y) on a given page.
    public func editorHitTest(editorId: String, pageIndex: UInt32, xPt: Float, yPt: Float) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorHitTest(editorId: editorId, pageIndex: pageIndex, xPt: xPt, yPt: yPt)
    }

    /// Toggles bold typography.
    public func editorToggleBold(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorToggleBold(editorId: editorId)
    }

    /// Toggles italic typography.
    public func editorToggleItalic(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorToggleItalic(editorId: editorId)
    }

    /// Sets heading level (1..6) or 0 for body text.
    public func editorSetHeadingLevel(editorId: String, level: UInt8) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorSetHeadingLevel(editorId: editorId, level: level)
    }

    /// Inserts an unordered bullet list block.
    public func editorInsertBulletList(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorInsertBulletList(editorId: editorId)
    }

    /// Inserts an ordered numbered list block.
    public func editorInsertNumberedList(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorInsertNumberedList(editorId: editorId)
    }

    /// Inserts a table block with specified rows and columns.
    public func editorInsertTable(editorId: String, rows: UInt32, cols: UInt32) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorInsertTable(editorId: editorId, rows: rows, cols: cols)
    }

    /// Inserts a footnote definition and inline reference.
    public func editorInsertFootnote(editorId: String, text: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorInsertFootnote(editorId: editorId, text: text)
    }

    /// Undoes the last mutation.
    public func editorUndo(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorUndo(editorId: editorId)
    }

    /// Redoes the undone mutation.
    public func editorRedo(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorRedo(editorId: editorId)
    }

    /// Saves document as a hardened `.pdoc` package.
    public func editorSavePdoc(editorId: String, targetPath: String?) throws -> String {
        try documentCoreEditorSavePdoc(editorId: editorId, targetPath: targetPath)
    }

    /// Measures typing and reflow latency in microseconds.
    public func editorMeasureTypingLatencyMicros(editorId: String, sampleText: String) throws -> UInt64 {
        try documentCoreEditorMeasureTypingLatencyMicros(editorId: editorId, sampleText: sampleText)
    }

    /// Enables change tracking under the given author name (`TRACK-001`).
    public func editorEnableTracking(editorId: String, author: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorEnableTracking(editorId: editorId, author: author)
    }

    /// Disables change tracking for subsequent edits (`TRACK-001`).
    public func editorDisableTracking(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorDisableTracking(editorId: editorId)
    }

    /// Adds a margin comment anchored to the active block and optional character range (`TRACK-005`).
    public func editorAddComment(editorId: String, anchorStart: UInt32?, anchorEnd: UInt32?, content: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorAddComment(editorId: editorId, anchorStart: anchorStart, anchorEnd: anchorEnd, content: content)
    }

    /// Appends a reply to an existing comment thread (`TRACK-005`).
    public func editorReplyComment(editorId: String, commentId: String, content: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorReplyComment(editorId: editorId, commentId: commentId, content: content)
    }

    /// Sets the resolved status of a comment thread (`TRACK-005`).
    public func editorResolveComment(editorId: String, commentId: String, resolved: Bool) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorResolveComment(editorId: editorId, commentId: commentId, resolved: resolved)
    }

    /// Deletes a comment thread completely (`TRACK-005`).
    public func editorDeleteComment(editorId: String, commentId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorDeleteComment(editorId: editorId, commentId: commentId)
    }

    /// Accepts an individual tracked change by ID (`TRACK-003`).
    public func editorAcceptChange(editorId: String, changeId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorAcceptChange(editorId: editorId, changeId: changeId)
    }

    /// Rejects an individual tracked change by ID (`TRACK-003`).
    public func editorRejectChange(editorId: String, changeId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorRejectChange(editorId: editorId, changeId: changeId)
    }

    /// Accepts all pending tracked changes across the document (`TRACK-003`).
    public func editorAcceptAllChanges(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorAcceptAllChanges(editorId: editorId)
    }

    /// Rejects all pending tracked changes across the document (`TRACK-003`).
    public func editorRejectAllChanges(editorId: String) throws -> FfiEditorSessionSnapshot {
        try documentCoreEditorRejectAllChanges(editorId: editorId)
    }

    /// Exports the flow document to a Tagged PDF/A-2b and PDF/UA-1 compliant stream (`EXPORT-001`).
    public func editorExportPdf(editorId: String, includeComments: Bool, includeTrackedChanges: Bool) throws -> Data {
        try documentCoreEditorExportPdf(editorId: editorId, includeComments: includeComments, includeTrackedChanges: includeTrackedChanges)
    }
}

