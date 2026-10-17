import Foundation
import Testing
import VocumentFFI
@testable import VocumentMacKit

@Test
func verifyMacPlatformUniFFIHandshakeAndPdfSession() throws {
    let client = DocumentCoreClient()
    let handshake = try client.performHandshake()
    #expect(handshake.baselineVersion == "0.1")
    #expect(handshake.milestone == "M0")
    #expect(handshake.strictOfflineDefault == true)
    #expect(handshake.telemetryEnabled == false)
    #expect(handshake.targetFps == 60)
    #expect(handshake.defaultGermanVariant == "de-AT")
    #expect(handshake.builtInOcrLanguages == ["deu", "eng"])

    let sampleURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/corpus/m0_1_sample_two_pages.pdf")

    let session = try client.openScopedDocument(at: sampleURL)
    #expect(session.documentId == "doc_1a00192b75fb403b")
    #expect(session.accessOrigin == .macOsScopedUrl)
    #expect(session.pdfSummary.pageCount == 2)
    #expect(session.pdfSummary.title == "Vocument M0.1 Sample Document")
    try client.closeSession(sessionId: session.sessionId)
}
