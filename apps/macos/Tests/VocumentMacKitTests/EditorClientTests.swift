import Testing
import Foundation
@testable import VocumentMacKit
import VocumentFFI

@Suite("M4.3 Native Platform Flow Editor Client Tests")
struct EditorClientTests {
    @Test("Editor session creation, text input, typography, and undo/redo")
    func verifyEditorBasicTypingAndUndo() throws {
        let client = DocumentCoreClient()
        let session = try client.createEditorSession(title: "BGB Schriftsatz")
        #expect(session.title == "BGB Schriftsatz")
        #expect(session.totalPages >= 1)

        let id = session.editorId

        // Insert text
        let s1 = try client.editorInsertText(editorId: id, text: "Gemäß § 242 BGB ist der Schuldner verpflichtet...")
        #expect(s1.plainText.contains("§ 242 BGB"))
        #expect(s1.canUndo == true)
        #expect(s1.canRedo == false)

        // Bold toggle
        _ = try client.editorToggleBold(editorId: id)
        let s2 = try client.editorInsertText(editorId: id, text: " Treu und Glauben.")
        #expect(s2.plainText.contains("Treu und Glauben"))

        // Undo
        let sUndo = try client.editorUndo(editorId: id)
        #expect(!sUndo.plainText.contains("Treu und Glauben"))
        #expect(sUndo.canRedo == true)

        // Redo
        let sRedo = try client.editorRedo(editorId: id)
        #expect(sRedo.plainText.contains("Treu und Glauben"))

        // Paragraph break
        let sBreak = try client.editorInsertParagraphBreak(editorId: id)
        #expect(!sBreak.plainText.isEmpty)

        // Set Heading
        _ = try client.editorInsertText(editorId: id, text: "I. Einleitung")
        let sH1 = try client.editorSetHeadingLevel(editorId: id, level: 1)
        #expect(sH1.plainText.contains("I. Einleitung"))
        #expect(sH1.plainText.contains("\n"))

        // Lists & Table & Footnote
        let sList = try client.editorInsertBulletList(editorId: id)
        #expect(!sList.plainText.isEmpty)

        let sTable = try client.editorInsertTable(editorId: id, rows: 2, cols: 2)
        #expect(sTable.totalPages >= 1)

        let sFn = try client.editorInsertFootnote(editorId: id, text: "Vgl. Palandt § 242 Rn. 1.")
        #expect(sFn.totalPages >= 1)

        // Coordinate hit test
        let sHit = try client.editorHitTest(editorId: id, pageIndex: 0, xPt: 120.0, yPt: 120.0)
        #expect(sHit.cursorRect != nil)

        // Performance budget: typing latency < 16,000 µs
        let latencyMicros = try client.editorMeasureTypingLatencyMicros(
            editorId: id,
            sampleText: "Echtzeit-Textverarbeitung im Rechtsverkehr."
        )
        #expect(latencyMicros < 16_000)

        // Atomic .pdoc save and reload
        let tmpPath = NSTemporaryDirectory() + "test_swift_editor_\(ProcessInfo.processInfo.processIdentifier).pdoc"
        let savedPath = try client.editorSavePdoc(editorId: id, targetPath: tmpPath)
        #expect(FileManager.default.fileExists(atPath: savedPath))

        try client.closeEditorSession(editorId: id)

        let reopened = try client.openPdocEditorSession(at: tmpPath)
        #expect(reopened.title == "BGB Schriftsatz")
        #expect(reopened.plainText.contains("§ 242 BGB"))

        try client.closeEditorSession(editorId: reopened.editorId)
        try? FileManager.default.removeItem(atPath: tmpPath)
    }

    @Test("Reviewing system: tracked changes, margin comments, and PDF/A export")
    func verifyReviewingTrackedChangesAndPdfExport() throws {
        let client = DocumentCoreClient()
        let session = try client.createEditorSession(title: "Vertragsentwurf 2026")
        let id = session.editorId

        _ = try client.editorInsertText(editorId: id, text: "Vertragsklausel 1: Gewährleistung beträgt ein Jahr.")

        // 1. Enable Tracking
        let sTrack = try client.editorEnableTracking(editorId: id, author: "Dr. Rechtsanwalt")
        #expect(sTrack.isTracking == true)

        // 2. Insert tracked text
        let sIns = try client.editorInsertText(editorId: id, text: " Verlängerung auf drei Jahre.")
        #expect(sIns.trackedChanges.count == 1)
        #expect(sIns.trackedChanges[0].author == "Dr. Rechtsanwalt")

        // 3. Add Margin Comment
        let sComm = try client.editorAddComment(editorId: id, anchorStart: 0, anchorEnd: 16, content: "Klausel an BGB anpassen.")
        #expect(sComm.comments.count == 1)
        let commentId = sComm.comments[0].commentId

        // 4. Reply to Comment
        let sRep = try client.editorReplyComment(editorId: id, commentId: commentId, content: "Wurde bereits mit Mandant abgestimmt.")
        #expect(sRep.comments[0].replies.count == 1)

        // 5. Resolve Comment
        let sRes = try client.editorResolveComment(editorId: id, commentId: commentId, resolved: true)
        #expect(sRes.comments[0].resolved == true)

        // 6. Accept change
        let sAcc = try client.editorAcceptChange(editorId: id, changeId: sIns.trackedChanges[0].changeId)
        #expect(sAcc.trackedChanges.isEmpty)

        // 7. Export Tagged PDF/A & PDF/UA
        let pdfData = try client.editorExportPdf(editorId: id, includeComments: true, includeTrackedChanges: true)
        #expect(pdfData.count > 500)
        let prefix = String(data: pdfData.prefix(8), encoding: .utf8)
        #expect(prefix == "%PDF-1.7")

        try client.closeEditorSession(editorId: id)
    }
}
