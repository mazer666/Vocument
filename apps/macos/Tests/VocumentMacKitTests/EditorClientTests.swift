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
}
