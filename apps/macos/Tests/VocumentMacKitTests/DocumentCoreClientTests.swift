import Foundation
import Testing
import VocumentFFI
@testable import VocumentMacKit

@Test
func verifyMacPlatformUniFFIHandshakePdfSessionAndRendering() throws {
    let client = DocumentCoreClient()
    let handshake = try client.performHandshake()
    #expect(handshake.baselineVersion == "0.1")
    #expect(handshake.milestone == "M3")
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

@Test
func verifyMacPlatformSanitizeDocument() throws {
    let client = DocumentCoreClient()
    let m03URL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/corpus/m0_3_navigation_and_search.pdf")
    let session = try client.openScopedDocument(at: m03URL)

    // Add a highlight first
    let highlight = FfiHighlightAnnotation(
        annotationId: "mac-hl-1",
        pageIndex: 0,
        rect: FfiPdfRectMilliPt(x0MilliPt: 100_000, y0MilliPt: 100_000, x1MilliPt: 200_000, y1MilliPt: 120_000),
        colorR: 255,
        colorG: 230,
        colorB: 80,
        opacityMilli: 450,
        author: "MacUser",
        contents: "Sanitize test note"
    )
    _ = try client.addHighlightAnnotation(sessionId: session.sessionId, annotation: highlight)

    // Sanitize document
    let record = try client.sanitizeDocument(sessionId: session.sessionId)
    #expect(record.description == "Sanitize document")

    // Annotations should now be cleared
    let annots = try client.pageHighlightAnnotations(sessionId: session.sessionId, pageIndex: 0)
    #expect(annots.isEmpty)

    try client.closeSession(sessionId: session.sessionId)
}

@Test
func verifyMacPlatformDocumentComparisonAndDiffing() throws {
    let client = DocumentCoreClient()

    let sampleURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/corpus/m0_1_sample_two_pages.pdf")

    let sessionA = try client.openScopedDocument(at: sampleURL)
    let sessionB = try client.openScopedDocument(at: sampleURL)

    let options = FfiComparisonOptions(
        granularity: .word,
        ignoreWhitespace: true,
        ignoreCase: false,
        visualPixelThreshold: 16,
        visualClusterDistancePt: 10.0,
        alignClauses: true
    )

    let report = try client.compareDocuments(
        sessionIdA: sessionA.sessionId,
        sessionIdB: sessionB.sessionId,
        options: options
    )

    #expect(report.pageCountA == 2)
    #expect(report.pageCountB == 2)
    #expect(report.summary.totalChanges == 0)
    #expect(report.summary.identicalPagesCount == 2)
    #expect(report.summary.similarityRatio >= 0.99)
    #expect(report.pageAlignments.count == 2)
    #expect(report.pageAlignments[0].pageA == 0)
    #expect(report.pageAlignments[0].pageB == 0)

    let textReport = client.compareTextStreams(
        titleA: "Version 1",
        textA: "§ 1 Die Vertragslaufzeit beträgt 12 Monate.",
        titleB: "Version 2",
        textB: "§ 1 Die Vertragslaufzeit beträgt 24 Monate.",
        options: options
    )
    #expect(textReport.summary.totalChanges == 1)
    #expect(textReport.summary.replacementsCount == 1)
    #expect(!textReport.markdownReport.isEmpty)
    #expect(!textReport.htmlReport.isEmpty)

    try client.closeSession(sessionId: sessionA.sessionId)
    try client.closeSession(sessionId: sessionB.sessionId)
}

