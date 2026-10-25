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
}
