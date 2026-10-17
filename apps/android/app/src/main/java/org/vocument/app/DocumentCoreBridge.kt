package org.vocument.app

import uniffi.vocument_ffi.DocumentCoreHandshake
import uniffi.vocument_ffi.FfiBrokeredFileDescriptor
import uniffi.vocument_ffi.FfiDocumentSessionSnapshot
import uniffi.vocument_ffi.FfiFileAccessOrigin
import uniffi.vocument_ffi.FfiHighlightAnnotation
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiPageDescriptor
import uniffi.vocument_ffi.FfiPageLabelRange
import uniffi.vocument_ffi.FfiPdfPageGeometry
import uniffi.vocument_ffi.FfiPdfRectMilliPt
import uniffi.vocument_ffi.FfiRenderedThumbnail
import uniffi.vocument_ffi.FfiRenderedTile
import uniffi.vocument_ffi.FfiSearchIndexProgress
import uniffi.vocument_ffi.FfiSearchResponse
import uniffi.vocument_ffi.FfiTextSelectionResult
import uniffi.vocument_ffi.FfiThumbnailCacheStats
import uniffi.vocument_ffi.FfiTileCacheStats
import uniffi.vocument_ffi.FfiTileRenderRequest
import uniffi.vocument_ffi.FfiTransactionHistorySnapshot
import uniffi.vocument_ffi.FfiViewportRenderResult
import uniffi.vocument_ffi.documentCoreAddHighlightAnnotation
import uniffi.vocument_ffi.documentCoreCloseSession
import uniffi.vocument_ffi.documentCoreExportTransactionJournalJson
import uniffi.vocument_ffi.documentCoreGetSessionSnapshot
import uniffi.vocument_ffi.documentCoreHandleMemoryPressure
import uniffi.vocument_ffi.documentCoreHandshake
import uniffi.vocument_ffi.documentCoreIndexSearchBatch
import uniffi.vocument_ffi.documentCoreOpenBrokeredFile
import uniffi.vocument_ffi.documentCorePageDescriptors
import uniffi.vocument_ffi.documentCorePageGeometry
import uniffi.vocument_ffi.documentCorePageHighlightAnnotations
import uniffi.vocument_ffi.documentCorePageRotationDegrees
import uniffi.vocument_ffi.documentCoreRedo
import uniffi.vocument_ffi.documentCoreRemoveHighlightAnnotation
import uniffi.vocument_ffi.documentCoreRenderPageThumbnail
import uniffi.vocument_ffi.documentCoreRenderViewportWithPrefetch
import uniffi.vocument_ffi.documentCoreRenderVisibleTile
import uniffi.vocument_ffi.documentCoreResolvePageNavigation
import uniffi.vocument_ffi.documentCoreRestoreTransactionJournalJson
import uniffi.vocument_ffi.documentCoreRotatePages
import uniffi.vocument_ffi.documentCoreSearchExact
import uniffi.vocument_ffi.documentCoreSearchIndexProgress
import uniffi.vocument_ffi.documentCoreSelectPageText
import uniffi.vocument_ffi.documentCoreSetPageLabelRanges
import uniffi.vocument_ffi.documentCoreThumbnailCacheStats
import uniffi.vocument_ffi.documentCoreTileCacheStats
import uniffi.vocument_ffi.documentCoreTransactionHistory
import uniffi.vocument_ffi.documentCoreUndo

/**
 * Thin Kotlin client wrapping the generated UniFFI `DocumentCore` bindings (`ARCHITECTURE.md` §§3–12, §15, §17).
 * Never duplicates configuration defaults or accesses MuPDF directly; all document state lives in `vocument-core`.
 */
class DocumentCoreBridge {
    fun performHandshake(): DocumentCoreHandshake {
        return documentCoreHandshake()
    }

    /**
     * Opens a PDF resolved through Android's Storage Access Framework (`content://` URI)
     * inside the Rust core (`ROADMAP.md` M0.1).
     */
    fun openSafDocument(
        safUriString: String,
        displayName: String,
        safDocumentId: String?,
        lastModifiedUnixMs: Long?,
        bytes: ByteArray
    ): FfiDocumentSessionSnapshot {
        val descriptor = FfiBrokeredFileDescriptor(
            sourceToken = safUriString,
            displayName = displayName,
            accessOrigin = FfiFileAccessOrigin.ANDROID_SAF_URI,
            osFileId = safDocumentId,
            modifiedUnixMs = lastModifiedUnixMs,
            bytes = bytes
        )
        return documentCoreOpenBrokeredFile(descriptor)
    }

    fun getSessionSnapshot(sessionId: String): FfiDocumentSessionSnapshot {
        return documentCoreGetSessionSnapshot(sessionId)
    }

    fun closeSession(sessionId: String) {
        documentCoreCloseSession(sessionId)
    }

    /**
     * Queries the unscaled geometry (`widthMilliPt`, `heightMilliPt`) of [pageIndex] via `DocumentActor` (`ROADMAP.md` M0.2).
     */
    fun pageGeometry(sessionId: String, pageIndex: UInt): FfiPdfPageGeometry {
        return documentCorePageGeometry(sessionId, pageIndex)
    }

    /**
     * Renders a single visible page or tile at `VisiblePageRender` priority (`ROADMAP.md` M0.2).
     */
    fun renderVisibleTile(sessionId: String, request: FfiTileRenderRequest): FfiRenderedTile {
        return documentCoreRenderVisibleTile(sessionId, request)
    }

    /**
     * Renders the visible viewport tile and schedules neighbouring-page prefetch (`ROADMAP.md` M0.2).
     */
    fun renderViewportWithPrefetch(
        sessionId: String,
        request: FfiTileRenderRequest
    ): FfiViewportRenderResult {
        return documentCoreRenderViewportWithPrefetch(sessionId, request)
    }

    /**
     * Forwards Android `ComponentCallbacks2` memory pressure signals (`Normal`, `Warning`, `Critical`)
     * into the adaptive `TileCache` and `ThumbnailCache` (`ARCHITECTURE.md` §9, `BIGMOBILE-001`).
     */
    fun handleMemoryPressure(level: FfiMemoryPressureLevel): FfiTileCacheStats {
        return documentCoreHandleMemoryPressure(level)
    }

    /**
     * Queries current telemetry statistics of the shared adaptive `TileCache`.
     */
    fun tileCacheStats(): FfiTileCacheStats {
        return documentCoreTileCacheStats()
    }

    /**
     * Returns all page descriptors (`pageIndex`, `physicalPageNumber`, `logicalLabel`) (`ROADMAP.md` M0.3, `PAGELABEL-001..004`).
     */
    fun pageDescriptors(sessionId: String): List<FfiPageDescriptor> {
        return documentCorePageDescriptors(sessionId)
    }

    /**
     * Resolves a user navigation input (`"ii"`, `"1"`, `"A-1"`, or `"#3"`) (`PAGELABEL-003`).
     */
    fun resolvePageNavigation(sessionId: String, query: String): FfiPageDescriptor {
        return documentCoreResolvePageNavigation(sessionId, query)
    }

    /**
     * Replaces the page-label ranges of a session and returns updated descriptors (`PAGELABEL-004`).
     */
    fun setPageLabelRanges(
        sessionId: String,
        ranges: List<FfiPageLabelRange>
    ): List<FfiPageDescriptor> {
        return documentCoreSetPageLabelRanges(sessionId, ranges)
    }

    /**
     * Renders or fetches a cached page thumbnail at `JobPriority::Thumbnails` (`PAGEVIEW-001`, `PAGEVIEW-002`).
     */
    fun renderPageThumbnail(
        sessionId: String,
        pageIndex: UInt,
        maxDimensionPx: UInt
    ): FfiRenderedThumbnail {
        return documentCoreRenderPageThumbnail(sessionId, pageIndex, maxDimensionPx)
    }

    /**
     * Queries current telemetry statistics of the separate `ThumbnailCache`.
     */
    fun thumbnailCacheStats(): FfiThumbnailCacheStats {
        return documentCoreThumbnailCacheStats()
    }

    /**
     * Selects page text (full page when [selectionRect] is `null`, or within a bounding box)
     * returning both raw and cleaned (`OCRCOPY-002`) plain text (`OCRCOPY-001`, `OCRCOPY-002`).
     */
    fun selectPageText(
        sessionId: String,
        pageIndex: UInt,
        selectionRect: FfiPdfRectMilliPt? = null
    ): FfiTextSelectionResult {
        return documentCoreSelectPageText(sessionId, pageIndex, selectionRect)
    }

    /**
     * Indexes up to [maxPages] unindexed pages into the session's progressive SQLite FTS5 index,
     * prioritizing [preferredStartPage] and nearby pages (`SEARCH-001`, `SEARCH-009`).
     */
    fun indexSearchBatch(
        sessionId: String,
        preferredStartPage: UInt,
        maxPages: UInt
    ): FfiSearchIndexProgress {
        return documentCoreIndexSearchBatch(sessionId, preferredStartPage, maxPages)
    }

    /**
     * Returns the current progressive SQLite FTS5 indexing progress (`SEARCH-009`).
     */
    fun searchIndexProgress(sessionId: String): FfiSearchIndexProgress {
        return documentCoreSearchIndexProgress(sessionId)
    }

    /**
     * Executes classic exact full-text search against the session's SQLite FTS5 index (`SEARCH-002`),
     * returning hits immediately even before full indexing completes (M0.3 Gate).
     */
    fun searchExact(
        sessionId: String,
        query: String
    ): FfiSearchResponse {
        return documentCoreSearchExact(sessionId, query)
    }

    /**
     * Queries the current rotation angle (`0`, `90`, `180`, `270`) of [pageIndex] (`ROTATE-001`, `ROADMAP.md` M0.4).
     */
    fun pageRotationDegrees(sessionId: String, pageIndex: UInt): UInt {
        return documentCorePageRotationDegrees(sessionId, pageIndex)
    }

    /**
     * Lists all standard PDF `/Highlight` annotations on [pageIndex] (`ANNOT-001`, `INTEROP-002`, `ROADMAP.md` M0.4).
     */
    fun pageHighlightAnnotations(
        sessionId: String,
        pageIndex: UInt
    ): List<FfiHighlightAnnotation> {
        return documentCorePageHighlightAnnotations(sessionId, pageIndex)
    }

    /**
     * Rotates one or more pages by [deltaDegrees] (`+90`, `-90`, `180`) via `TransactionManager` (`ROTATE-001`, `PAGE-001`, `ROADMAP.md` M0.4 Gate).
     */
    fun rotatePages(
        sessionId: String,
        pageIndices: List<UInt>,
        deltaDegrees: Int
    ): FfiTransactionHistorySnapshot {
        return documentCoreRotatePages(sessionId, pageIndices, deltaDegrees)
    }

    /**
     * Adds a standard PDF `/Highlight` annotation via `TransactionManager` (`ANNOT-001`, `INTEROP-002`, `ROADMAP.md` M0.4 Gate).
     */
    fun addHighlightAnnotation(
        sessionId: String,
        annotation: FfiHighlightAnnotation
    ): FfiTransactionHistorySnapshot {
        return documentCoreAddHighlightAnnotation(sessionId, annotation)
    }

    /**
     * Removes a standard PDF `/Highlight` annotation by [annotationId] via `TransactionManager` (`ANNOT-001`).
     */
    fun removeHighlightAnnotation(
        sessionId: String,
        pageIndex: UInt,
        annotationId: String
    ): FfiTransactionHistorySnapshot {
        return documentCoreRemoveHighlightAnnotation(sessionId, pageIndex, annotationId)
    }

    /**
     * Undoes the last committed transaction on [sessionId] via `TransactionManager` (`UNDO-001`, `UNDO-004`).
     */
    fun undo(sessionId: String): FfiTransactionHistorySnapshot {
        return documentCoreUndo(sessionId)
    }

    /**
     * Redoes the last undone transaction on [sessionId] via `TransactionManager` (`UNDO-001`, `UNDO-004`).
     */
    fun redo(sessionId: String): FfiTransactionHistorySnapshot {
        return documentCoreRedo(sessionId)
    }

    /**
     * Queries the live Undo/Redo and transaction history state for [sessionId] (`UNDO-001`, `UNDO-002`).
     */
    fun transactionHistory(sessionId: String): FfiTransactionHistorySnapshot {
        return documentCoreTransactionHistory(sessionId)
    }

    /**
     * Exports the verified `PersistentTransactionJournal` as pretty-printed JSON (`ARCHITECTURE.md` §§11–12).
     */
    fun exportTransactionJournalJson(sessionId: String): String {
        return documentCoreExportTransactionJournalJson(sessionId)
    }

    /**
     * Restores and replays a `PersistentTransactionJournal` JSON payload onto [sessionId] (`UNDO-001`).
     */
    fun restoreTransactionJournalJson(
        sessionId: String,
        journalJson: String
    ): FfiTransactionHistorySnapshot {
        return documentCoreRestoreTransactionJournalJson(sessionId, journalJson)
    }

    /**
     * Converts an RGBA8 `FfiRenderedTile` byte array into an `IntArray` of `ARGB_8888` pixels
     * suitable for `android.graphics.Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888)`.
     */
    fun decodeRgba8ToArgbPixels(tile: FfiRenderedTile): IntArray {
        return decodeRawRgba8(tile.widthPx.toInt(), tile.heightPx.toInt(), tile.rgbaBytes)
    }

    /**
     * Converts an RGBA8 `FfiRenderedThumbnail` byte array into an `IntArray` of `ARGB_8888` pixels.
     */
    fun decodeThumbnailToArgbPixels(thumbnail: FfiRenderedThumbnail): IntArray {
        return decodeRawRgba8(thumbnail.widthPx.toInt(), thumbnail.heightPx.toInt(), thumbnail.rgbaBytes)
    }

    private fun decodeRawRgba8(width: Int, height: Int, rgba: ByteArray): IntArray {
        val out = IntArray(width * height)
        var byteOffset = 0
        for (i in out.indices) {
            val r = rgba[byteOffset].toInt() and 0xFF
            val g = rgba[byteOffset + 1].toInt() and 0xFF
            val b = rgba[byteOffset + 2].toInt() and 0xFF
            val a = rgba[byteOffset + 3].toInt() and 0xFF
            out[i] = (a shl 24) or (r shl 16) or (g shl 8) or b
            byteOffset += 4
        }
        return out
    }
}
