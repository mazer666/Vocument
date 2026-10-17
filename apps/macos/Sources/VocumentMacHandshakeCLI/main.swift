import Foundation
import VocumentMacKit
import VocumentFFI

@main
struct VocumentMacHandshakeCLI {
    static func main() throws {
        let client = DocumentCoreClient()
        let handshake = try client.performHandshake()

        guard handshake.baselineVersion == "0.1" else {
            fatalError("Expected baselineVersion '0.1', got '\(handshake.baselineVersion)'")
        }
        guard handshake.milestone == "M0" else {
            fatalError("Expected milestone 'M0', got '\(handshake.milestone)'")
        }
        guard handshake.strictOfflineDefault == true else {
            fatalError("Expected strictOfflineDefault == true (NET-001)")
        }
        guard handshake.telemetryEnabled == false else {
            fatalError("Expected telemetryEnabled == false (LOG-001)")
        }
        guard handshake.targetFps == 60 else {
            fatalError("Expected targetFps == 60 (PERF-001), got \(handshake.targetFps)")
        }
        guard handshake.defaultGermanVariant == "de-AT" else {
            fatalError("Expected defaultGermanVariant == 'de-AT' (I18N-002), got '\(handshake.defaultGermanVariant)'")
        }
        guard handshake.builtInOcrLanguages == ["deu", "eng"] else {
            fatalError("Expected builtInOcrLanguages == ['deu', 'eng'] (OCR-002), got \(handshake.builtInOcrLanguages)")
        }

        print("✓ Swift → UniFFI → Rust DocumentCore handshake verified: baseline=\(handshake.baselineVersion), milestone=\(handshake.milestone), offline=\(handshake.strictOfflineDefault), telemetry=\(handshake.telemetryEnabled), fps=\(handshake.targetFps)")

        // M0.1 verification: open real PDF via macOS ScopedLocalFileBroker → DocumentSession → MuPDF
        let samplePdfURL = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("tests/corpus/m0_1_sample_two_pages.pdf")
        let session = try client.openScopedDocument(at: samplePdfURL)

        guard session.sessionId.hasPrefix("sess_") else {
            fatalError("Expected session_id prefix 'sess_', got '\(session.sessionId)'")
        }
        guard session.documentId == "doc_1a00192b75fb403b" else {
            fatalError("Expected stable document_id 'doc_1a00192b75fb403b', got '\(session.documentId)'")
        }
        guard session.accessOrigin == .macOsScopedUrl else {
            fatalError("Expected accessOrigin == .macOsScopedUrl")
        }
        guard session.fileIdentity.sizeBytes == 697 else {
            fatalError("Expected sizeBytes == 697, got \(session.fileIdentity.sizeBytes)")
        }
        guard session.pdfSummary.pageCount == 2 else {
            fatalError("Expected pageCount == 2, got \(session.pdfSummary.pageCount)")
        }
        guard session.pdfSummary.title == "Vocument M0.1 Sample Document" else {
            fatalError("Unexpected title: \(String(describing: session.pdfSummary.title))")
        }
        guard session.pdfSummary.author == "Vocument Engineering" else {
            fatalError("Unexpected author: \(String(describing: session.pdfSummary.author))")
        }
        guard session.pdfSummary.engineIdentifier.hasPrefix("mupdf-1.") else {
            fatalError("Unexpected engineIdentifier: \(session.pdfSummary.engineIdentifier)")
        }

        let fetched = try client.sessionSnapshot(sessionId: session.sessionId)
        guard fetched == session else {
            fatalError("Fetched session snapshot did not match opened session")
        }
        try client.closeSession(sessionId: session.sessionId)

        print("✓ Swift (macOS M0.1) → FileBroker → DocumentSession → MuPDF verified: session=\(session.sessionId), doc=\(session.documentId), pages=\(session.pdfSummary.pageCount), title='\(session.pdfSummary.title ?? "")', engine=\(session.pdfSummary.engineIdentifier)")
    }
}
