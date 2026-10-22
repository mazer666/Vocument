import Foundation
import Testing
import VocumentFFI
@testable import VocumentMacKit

@Test
func verifyMacPlatformUniFFIHandshakePdfSessionAndRendering() throws {
    let client = DocumentCoreClient()
    let handshake = try client.performHandshake()
    #expect(handshake.baselineVersion == "0.1")
    #expect(handshake.milestone == "M2")
    #expect(handshake.strictOfflineDefault == true)
    #expect(handshake.telemetryEnabled == false)
    #expect(handshake.targetFps == 60)
    #expect(handshake.defaultGermanVariant == "de-AT")
    #expect(handshake.builtInOcrLanguages == ["deu", "eng"])

    let _ = try client.handleMemoryPressure(level: .critical)
    let _ = try client.handleMemoryPressure(level: .normal)

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

    let geo = try client.pageGeometry(sessionId: session.sessionId, pageIndex: 0)
    #expect(geo.widthMilliPt == 595_000)
    #expect(geo.heightMilliPt == 842_000)

    let viewport = try client.renderViewportWithPrefetch(
        sessionId: session.sessionId,
        request: FfiTileRenderRequest(
            pageIndex: 0,
            scaleMilli: 1000,
            tileXPx: 0,
            tileYPx: 0,
            tileWidthPx: 595,
            tileHeightPx: 842
        )
    )
    #expect(viewport.visibleTile.widthPx == 595)
    #expect(viewport.visibleTile.heightPx == 842)
    #expect(viewport.prefetchedPages == [1])
    // The conversion of the tile into a `CGImage` belongs to macOS.
    #if canImport(CoreGraphics)
    let cgImage = DocumentCoreClient.makeCGImage(from: viewport.visibleTile)
    #expect(cgImage != nil)
    #endif

    let criticalStats = try client.handleMemoryPressure(level: .critical)
    #expect(criticalStats.cachedTileCount == 0)
    let _ = try client.handleMemoryPressure(level: .normal)

    try client.closeSession(sessionId: session.sessionId)

    let m03URL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/corpus/m0_3_navigation_and_search.pdf")
    let m03Session = try client.openScopedDocument(at: m03URL)
    let descriptors = try client.pageDescriptors(sessionId: m03Session.sessionId)
    #expect(descriptors.map(\.logicalLabel) == ["i", "ii", "1", "2"])

    let navTarget = try client.resolvePageNavigation(sessionId: m03Session.sessionId, query: "1")
    #expect(navTarget.pageIndex == 2)

    let thumb = try client.renderPageThumbnail(sessionId: m03Session.sessionId, pageIndex: 0, maxDimensionPx: 128)
    #expect(thumb.heightPx == 128)
    #if canImport(CoreGraphics)
    #expect(DocumentCoreClient.makeCGImage(from: thumb) != nil)
    #endif

    let selection = try client.selectPageText(sessionId: m03Session.sessionId, pageIndex: 1, selectionRect: nil)
    #expect(selection.cleanedText.contains("trustworthy"))

    let progress = try client.indexSearchBatch(sessionId: m03Session.sessionId, preferredStartPage: 2, maxPages: 1)
    #expect(progress.indexedPages == 1)
    #expect(progress.isComplete == false)

    let searchResponse = try client.searchExact(sessionId: m03Session.sessionId, query: "SQLite FTS5")
    #expect(searchResponse.progress.isComplete == false)
    #expect(searchResponse.hits.count == 1)
    #expect(searchResponse.hits[0].pageIndex == 2)

    try client.closeSession(sessionId: m03Session.sessionId)
}
