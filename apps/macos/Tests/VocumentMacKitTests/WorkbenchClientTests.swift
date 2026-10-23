import Foundation
import Testing
import VocumentFFI
@testable import VocumentMacKit

@Test
func verifyMacPlatformWorkbenchOperationsAndUndoRedo() throws {
    let client = DocumentCoreClient()

    let sampleURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/corpus/m0_1_sample_two_pages.pdf")

    let session = try client.openScopedDocument(at: sampleURL)
    #expect(session.pdfSummary.pageCount == 2)

    // 1. Preflight Audit
    let preflight = try client.workbenchPreflight(sessionId: session.sessionId)
    #expect(preflight.isSigned == false)
    #expect(preflight.allowsAssembly == true)

    // 2. Duplicate Page 0 -> page count 3
    let dupTx = try client.duplicatePage(sessionId: session.sessionId, pageIndex: 0)
    #expect(dupTx.description == "Duplicate page #1")

    let dupSnap = try client.sessionSnapshot(sessionId: session.sessionId)
    #expect(dupSnap.pdfSummary.pageCount == 3)

    // Test Undo of Duplication
    let undoHist = try client.undo(sessionId: session.sessionId)
    #expect(undoHist.canRedo == true)
    let undoSnap = try client.sessionSnapshot(sessionId: session.sessionId)
    #expect(undoSnap.pdfSummary.pageCount == 2)

    // Test Redo of Duplication
    let redoHist = try client.redo(sessionId: session.sessionId)
    #expect(redoHist.canUndo == true)
    let redoSnap = try client.sessionSnapshot(sessionId: session.sessionId)
    #expect(redoSnap.pdfSummary.pageCount == 3)

    // 3. Reorder Pages: [2, 0, 1]
    let reorderTx = try client.reorderPages(sessionId: session.sessionId, order: [2, 0, 1])
    #expect(reorderTx.description == "Reorder 3 pages")
    let reorderSnap = try client.sessionSnapshot(sessionId: session.sessionId)
    #expect(reorderSnap.pdfSummary.pageCount == 3)

    // 4. Extract Pages: extract page 0
    let extractedData = try client.extractPages(sessionId: session.sessionId, pageIndices: [0])
    #expect(extractedData.count > 100)
    let headerStr = String(data: extractedData.prefix(8), encoding: .ascii) ?? ""
    #expect(headerStr.starts(with: "%PDF-"))

    // 5. Delete Pages: delete page 1 and page 2 -> leaves 1 page
    let delTx = try client.deletePages(sessionId: session.sessionId, pageIndices: [1, 2])
    #expect(delTx.description == "Delete 2 page(s)")
    let delSnap = try client.sessionSnapshot(sessionId: session.sessionId)
    #expect(delSnap.pdfSummary.pageCount == 1)

    // Undo deletion -> back to 3 pages
    let undoDelHist = try client.undo(sessionId: session.sessionId)
    #expect(undoDelHist.canRedo == true)
    let recoveredSnap = try client.sessionSnapshot(sessionId: session.sessionId)
    #expect(recoveredSnap.pdfSummary.pageCount == 3)

    // 6. Rotate Pages: rotate page 0 by 90 degrees
    let rotHist = try client.rotatePages(sessionId: session.sessionId, pageIndices: [0], deltaDegrees: 90)
    #expect(rotHist.canUndo == true)

    try client.closeSession(sessionId: session.sessionId)
}
