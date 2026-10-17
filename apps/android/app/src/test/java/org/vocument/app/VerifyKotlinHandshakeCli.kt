package org.vocument.app

import java.io.File
import uniffi.vocument_ffi.FfiFileAccessOrigin
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiTileRenderRequest

fun main() {
    val bridge = DocumentCoreBridge()
    val handshake = bridge.performHandshake()

    check(handshake.baselineVersion == "0.1") {
        "Expected baselineVersion '0.1', got '${handshake.baselineVersion}'"
    }
    check(handshake.milestone == "M0") {
        "Expected milestone 'M0', got '${handshake.milestone}'"
    }
    check(handshake.strictOfflineDefault) {
        "Expected strictOfflineDefault == true (NET-001)"
    }
    check(!handshake.telemetryEnabled) {
        "Expected telemetryEnabled == false (LOG-001)"
    }
    check(handshake.targetFps == 60u) {
        "Expected targetFps == 60 (PERF-001), got ${handshake.targetFps}"
    }
    check(handshake.defaultGermanVariant == "de-AT") {
        "Expected defaultGermanVariant == 'de-AT' (I18N-002), got '${handshake.defaultGermanVariant}'"
    }
    check(handshake.builtInOcrLanguages == listOf("deu", "eng")) {
        "Expected builtInOcrLanguages == ['deu', 'eng'] (OCR-002), got ${handshake.builtInOcrLanguages}"
    }

    println(
        "✓ Kotlin (Android) → UniFFI → Rust DocumentCore handshake verified: " +
            "baseline=${handshake.baselineVersion}, milestone=${handshake.milestone}, " +
            "offline=${handshake.strictOfflineDefault}, telemetry=${handshake.telemetryEnabled}, " +
            "fps=${handshake.targetFps}"
    )

    bridge.handleMemoryPressure(FfiMemoryPressureLevel.CRITICAL)
    bridge.handleMemoryPressure(FfiMemoryPressureLevel.NORMAL)

    // M0.1 verification: open real PDF via Android SAF FileBroker → DocumentSession → MuPDF
    val openStartNs = System.nanoTime()
    val sampleFile = File("tests/corpus/m0_1_sample_two_pages.pdf")
    val pdfBytes = sampleFile.readBytes()
    val session = bridge.openSafDocument(
        safUriString = "content://com.android.providers.downloads.documents/document/m0_1_sample",
        displayName = sampleFile.name,
        safDocumentId = "downloads:m0_1_sample",
        lastModifiedUnixMs = sampleFile.lastModified(),
        bytes = pdfBytes
    )

    check(session.sessionId.startsWith("sess_")) {
        "Expected session_id prefix 'sess_', got '${session.sessionId}'"
    }
    check(session.documentId == "doc_1a00192b75fb403b") {
        "Expected stable document_id 'doc_1a00192b75fb403b', got '${session.documentId}'"
    }
    check(session.accessOrigin == FfiFileAccessOrigin.ANDROID_SAF_URI) {
        "Expected accessOrigin == ANDROID_SAF_URI, got ${session.accessOrigin}"
    }
    check(session.fileIdentity.sizeBytes == 697uL) {
        "Expected sizeBytes == 697, got ${session.fileIdentity.sizeBytes}"
    }
    check(session.pdfSummary.pageCount == 2u) {
        "Expected pageCount == 2, got ${session.pdfSummary.pageCount}"
    }
    check(session.pdfSummary.title == "Vocument M0.1 Sample Document") {
        "Unexpected title: ${session.pdfSummary.title}"
    }
    check(session.pdfSummary.author == "Vocument Engineering") {
        "Unexpected author: ${session.pdfSummary.author}"
    }
    check(session.pdfSummary.engineIdentifier.startsWith("mupdf-1.")) {
        "Unexpected engineIdentifier: ${session.pdfSummary.engineIdentifier}"
    }

    val fetched = bridge.getSessionSnapshot(session.sessionId)
    check(fetched == session) {
        "Fetched session snapshot did not match opened session"
    }

    println(
        "✓ Kotlin (Android M0.1) → SAF FileBroker → DocumentSession → MuPDF verified: " +
            "session=${session.sessionId}, doc=${session.documentId}, " +
            "pages=${session.pdfSummary.pageCount}, title='${session.pdfSummary.title}', " +
            "engine=${session.pdfSummary.engineIdentifier}"
    )

    // M0.2 verification: DocumentActor geometry, viewport render + prefetch, ARGB decode, zoom, and memory pressure
    val geo = bridge.pageGeometry(session.sessionId, 0u)
    check(geo.widthMilliPt == 595_000u && geo.heightMilliPt == 842_000u) {
        "Expected A4 page geometry 595000x842000 milli-pt, got ${geo.widthMilliPt}x${geo.heightMilliPt}"
    }

    val viewport = bridge.renderViewportWithPrefetch(
        session.sessionId,
        FfiTileRenderRequest(
            pageIndex = 0u,
            scaleMilli = 1000u,
            tileXPx = 0u,
            tileYPx = 0u,
            tileWidthPx = 595u,
            tileHeightPx = 842u
        )
    )
    val firstPageMs = (System.nanoTime() - openStartNs) / 1_000_000.0
    check(firstPageMs < 300.0) {
        "First-page open+render exceeded 300 ms budget: $firstPageMs ms"
    }
    check(viewport.visibleTile.widthPx == 595u && viewport.visibleTile.heightPx == 842u) {
        "Unexpected rendered tile dimensions"
    }
    check(viewport.prefetchedPages == listOf(1u)) {
        "Expected prefetchedPages == [1], got ${viewport.prefetchedPages}"
    }
    val argbPixels = bridge.decodeRgba8ToArgbPixels(viewport.visibleTile)
    check(argbPixels.size == 595 * 842) {
        "Expected 500990 ARGB pixels, got ${argbPixels.size}"
    }

    // Scroll to page 1 (must hit prefetched tile in TileCache)
    val hitsBefore = viewport.cacheStats.hitCount
    val page1Tile = bridge.renderVisibleTile(
        session.sessionId,
        FfiTileRenderRequest(
            pageIndex = 1u,
            scaleMilli = 1000u,
            tileXPx = 0u,
            tileYPx = 0u,
            tileWidthPx = 595u,
            tileHeightPx = 842u
        )
    )
    check(page1Tile.pageIndex == 1u) { "Expected pageIndex == 1" }
    val statsAfterHit = bridge.tileCacheStats()
    check(statsAfterHit.hitCount == hitsBefore + 1uL) {
        "Expected TileCache hit on prefetched page 1"
    }

    // Zoom to 200% (scaleMilli = 2000) sub-tile
    val zoomedTile = bridge.renderVisibleTile(
        session.sessionId,
        FfiTileRenderRequest(
            pageIndex = 0u,
            scaleMilli = 2000u,
            tileXPx = 0u,
            tileYPx = 0u,
            tileWidthPx = 256u,
            tileHeightPx = 256u
        )
    )
    check(zoomedTile.widthPx == 256u && zoomedTile.heightPx == 256u) {
        "Expected 256x256 zoomed tile"
    }

    // Memory pressure Warning + Critical verification
    val warningStats = bridge.handleMemoryPressure(FfiMemoryPressureLevel.WARNING)
    check(warningStats.pressureLevel == FfiMemoryPressureLevel.WARNING) {
        "Expected pressureLevel == WARNING"
    }
    val criticalStats = bridge.handleMemoryPressure(FfiMemoryPressureLevel.CRITICAL)
    check(criticalStats.cachedTileCount == 0u && criticalStats.currentUsedBytes == 0uL) {
        "Expected critical memory pressure to purge TileCache"
    }
    bridge.handleMemoryPressure(FfiMemoryPressureLevel.NORMAL)

    bridge.closeSession(session.sessionId)

    val formattedMs = String.format(java.util.Locale.ROOT, "%.2f", firstPageMs)
    println(
        "✓ Kotlin (Android M0.2) → DocumentActor → DisplayList → TileCache → ARGB Bitmap verified: " +
            "first_page_ms=$formattedMs, tile=595x842, prefetched=${viewport.prefetchedPages}, " +
            "zoom=200%, memory_pressure=ok"
    )
}
