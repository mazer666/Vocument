package org.vocument.app

import uniffi.vocument_ffi.*

/**
 * Editor extension methods for [DocumentCoreBridge] providing M4.3 native flow editor operations (`ADR-0020`, `UI-009`).
 */
fun DocumentCoreBridge.createEditorSession(title: String): FfiEditorSessionSnapshot {
    return documentCoreCreateEditorSession(title)
}

fun DocumentCoreBridge.openPdocEditorSession(filePath: String): FfiEditorSessionSnapshot {
    return documentCoreOpenPdocEditorSession(filePath)
}

fun DocumentCoreBridge.closeEditorSession(editorId: String) {
    documentCoreCloseEditorSession(editorId)
}

fun DocumentCoreBridge.editorInsertText(editorId: String, text: String): FfiEditorSessionSnapshot {
    return documentCoreEditorInsertText(editorId, text)
}

fun DocumentCoreBridge.editorInsertParagraphBreak(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorInsertParagraphBreak(editorId)
}

fun DocumentCoreBridge.editorDeleteBackward(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorDeleteBackward(editorId)
}

fun DocumentCoreBridge.editorDeleteForward(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorDeleteForward(editorId)
}

fun DocumentCoreBridge.editorSetCursor(editorId: String, blockId: String, offset: UInt): FfiEditorSessionSnapshot {
    return documentCoreEditorSetCursor(editorId, blockId, offset)
}

fun DocumentCoreBridge.editorSetSelection(
    editorId: String,
    anchorBlockId: String,
    anchorOffset: UInt,
    focusBlockId: String,
    focusOffset: UInt
): FfiEditorSessionSnapshot {
    return documentCoreEditorSetSelection(editorId, anchorBlockId, anchorOffset, focusBlockId, focusOffset)
}

fun DocumentCoreBridge.editorClearSelection(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorClearSelection(editorId)
}

fun DocumentCoreBridge.editorHitTest(editorId: String, pageIndex: UInt, xPt: Float, yPt: Float): FfiEditorSessionSnapshot {
    return documentCoreEditorHitTest(editorId, pageIndex, xPt, yPt)
}

fun DocumentCoreBridge.editorToggleBold(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorToggleBold(editorId)
}

fun DocumentCoreBridge.editorToggleItalic(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorToggleItalic(editorId)
}

fun DocumentCoreBridge.editorSetHeadingLevel(editorId: String, level: UByte): FfiEditorSessionSnapshot {
    return documentCoreEditorSetHeadingLevel(editorId, level)
}

fun DocumentCoreBridge.editorInsertBulletList(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorInsertBulletList(editorId)
}

fun DocumentCoreBridge.editorInsertNumberedList(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorInsertNumberedList(editorId)
}

fun DocumentCoreBridge.editorInsertTable(editorId: String, rows: UInt, cols: UInt): FfiEditorSessionSnapshot {
    return documentCoreEditorInsertTable(editorId, rows, cols)
}

fun DocumentCoreBridge.editorInsertFootnote(editorId: String, text: String): FfiEditorSessionSnapshot {
    return documentCoreEditorInsertFootnote(editorId, text)
}

fun DocumentCoreBridge.editorUndo(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorUndo(editorId)
}

fun DocumentCoreBridge.editorRedo(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorRedo(editorId)
}

fun DocumentCoreBridge.editorSavePdoc(editorId: String, targetPath: String?): String {
    return documentCoreEditorSavePdoc(editorId, targetPath)
}

fun DocumentCoreBridge.editorMeasureTypingLatencyMicros(editorId: String, sampleText: String): ULong {
    return documentCoreEditorMeasureTypingLatencyMicros(editorId, sampleText)
}

fun DocumentCoreBridge.editorEnableTracking(editorId: String, author: String): FfiEditorSessionSnapshot {
    return documentCoreEditorEnableTracking(editorId, author)
}

fun DocumentCoreBridge.editorDisableTracking(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorDisableTracking(editorId)
}

fun DocumentCoreBridge.editorAddComment(editorId: String, anchorStart: UInt?, anchorEnd: UInt?, content: String): FfiEditorSessionSnapshot {
    return documentCoreEditorAddComment(editorId, anchorStart, anchorEnd, content)
}

fun DocumentCoreBridge.editorReplyComment(editorId: String, commentId: String, content: String): FfiEditorSessionSnapshot {
    return documentCoreEditorReplyComment(editorId, commentId, content)
}

fun DocumentCoreBridge.editorResolveComment(editorId: String, commentId: String, resolved: Boolean): FfiEditorSessionSnapshot {
    return documentCoreEditorResolveComment(editorId, commentId, resolved)
}

fun DocumentCoreBridge.editorDeleteComment(editorId: String, commentId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorDeleteComment(editorId, commentId)
}

fun DocumentCoreBridge.editorAcceptChange(editorId: String, changeId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorAcceptChange(editorId, changeId)
}

fun DocumentCoreBridge.editorRejectChange(editorId: String, changeId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorRejectChange(editorId, changeId)
}

fun DocumentCoreBridge.editorAcceptAllChanges(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorAcceptAllChanges(editorId)
}

fun DocumentCoreBridge.editorRejectAllChanges(editorId: String): FfiEditorSessionSnapshot {
    return documentCoreEditorRejectAllChanges(editorId)
}

fun DocumentCoreBridge.editorExportPdf(editorId: String, includeComments: Boolean, includeTrackedChanges: Boolean): ByteArray {
    return documentCoreEditorExportPdf(editorId, includeComments, includeTrackedChanges)
}

