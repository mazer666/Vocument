package org.vocument.app

import uniffi.vocument_ffi.*

/**
 * Thin Kotlin client wrapping the generated UniFFI `DocumentCore` bindings (`ARCHITECTURE.md` §§3–12, §15, §17).
 * Never duplicates configuration defaults or accesses MuPDF directly; all document state lives in `vocument-core`.
 */
class DocumentCoreBridge {
    fun performHandshake(): DocumentCoreHandshake {
        return documentCoreHandshake()
    }

    /**
     * Configures the explicit filesystem path to the tessdata models directory.
     */
    fun configureTessdataPath(path: String) {
        documentCoreConfigureTessdataPath(path)
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

    /**
     * Opens a local PDF file path via `ScopedLocalFileBroker`.
     */
    fun openLocalDocument(filePath: String): FfiDocumentSessionSnapshot {
        return documentCoreOpenMacosScopedFile(filePath)
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
     * Safe save of active [sessionId] back to its original file (`ROADMAP.md` M0.5, `SAVE-001`).
     */
    /**
     * Reports the energy and thermal state of the device to the shared core
     * (`POWER-001`).
     */
    fun reportPowerState(state: FfiPowerState): FfiPowerLimits {
        return documentCoreReportPowerState(state)
    }

    /**
     * Returns the current bytes of the document.
     *
     * The shell writes them into a location that the user owns: the Android
     * document provider (`content://` URI) or a share sheet. The core has no
     * access to such a location.
     */
    fun exportSessionBytes(sessionId: String): ByteArray {
        return documentCoreExportSessionBytes(sessionId)
    }

    fun saveSession(sessionId: String): FfiSaveOutcome {
        return documentCoreSaveSession(sessionId)
    }

    /**
     * 'Save As' of active [sessionId] to [targetToken] (`SAVE-002`).
     */
    fun saveSessionAs(sessionId: String, targetToken: String): FfiSaveOutcome {
        return documentCoreSaveSessionAs(sessionId, targetToken)
    }

    /**
     * Saves a collision-free safe copy of [sessionId] (`ARCHITECTURE.md` §16, `FILELOCK-004`).
     */
    fun saveSessionCollisionFreeCopy(sessionId: String): FfiSaveSuccessSnapshot {
        return documentCoreSaveSessionCollisionFreeCopy(sessionId)
    }

    /**
     * Inspects whether on-disk file for [sessionId] was modified externally (`FILELOCK-002`).
     */
    fun checkExternalChanges(sessionId: String): FfiExternalChangeCheckResult {
        return documentCoreCheckExternalChanges(sessionId)
    }

    /**
     * Resolves an external save conflict according to [resolution] (`ARCHITECTURE.md` §16, `FILELOCK-004`).
     */
    fun resolveSaveConflict(
        sessionId: String,
        resolution: FfiSaveConflictResolution
    ): FfiSaveSuccessSnapshot? {
        return documentCoreResolveSaveConflict(sessionId, resolution)
    }

    /**
     * Generates a collision-free file path for [basePath] and [suffix] (`FILELOCK-004`).
     */
    fun generateCollisionFreePath(basePath: String, suffix: String): String {
        return documentCoreGenerateCollisionFreePath(basePath, suffix)
    }

    /**
     * Scans the recovery store and returns recoverable documents that experienced an unclean shutdown (`ROADMAP.md` M0.6).
     */
    fun scanRecoverableDocuments(): List<FfiRecoverableDocumentInfo> {
        return documentCoreScanRecoverableDocuments()
    }

    /**
     * Checks whether a specific document has a pending crash recovery container (`ROADMAP.md` M0.6).
     */
    fun checkDocumentRecovery(sourceToken: String): FfiRecoverableDocumentInfo? {
        return documentCoreCheckDocumentRecovery(sourceToken)
    }

    /**
     * Executes a recovery action (`RESTORE`, `PREVIEW`, `DISCARD`, `OPEN_AS_NEW`) on [documentId] (`RECOVERY-001`).
     *
     * **M0.6 Gate Invariant:** Disk file remains completely intact until explicit user save.
     */
    fun executeRecoveryAction(
        documentId: String,
        action: FfiRecoveryAction
    ): FfiRecoveryActionResult {
        return documentCoreExecuteRecoveryAction(documentId, action)
    }

    /**
     * Marks a document's recovery container clean shutdown.
     */
    fun markRecoveryCleanShutdown(documentId: String) {
        documentCoreMarkRecoveryCleanShutdown(documentId)
    }

    /**
     * Executes the full M0 DoD workflow and verifies generated PDF files across reference and foreign viewers (`ROADMAP.md` M0.7, `M0-ACCEPTANCE.md`).
     */
    fun runInteropGateSuite(): FfiInteropReport {
        return documentCoreRunInteropGateSuite()
    }

    /**
     * Detects document edges and corner coordinates in real time from a live camera frame (`SCAN-001`, `SCAN-004`).
     */
    fun detectDocumentEdges(frame: FfiScanFrame): FfiQuadCorners? {
        return documentCoreDetectDocumentEdges(frame)
    }

    /**
     * Starts a new multi-page scan capture session (`SCAN-006`).
     */
    fun startScanSession(): String {
        return documentCoreStartScanSession()
    }

    /**
     * Appends a captured camera scan frame to an active scanning session (`SCAN-006`).
     */
    fun appendScanPage(
        sessionId: String,
        frame: FfiScanFrame,
        corners: FfiQuadCorners? = null,
        profile: FfiScanCleanupProfile? = null
    ): FfiCapturedScanPage {
        return documentCoreAppendScanPage(sessionId, frame, corners, profile)
    }

    /**
     * Queries the current snapshot of an active scanning session.
     */
    fun getScanSession(sessionId: String): FfiScanSessionSnapshot {
        return documentCoreGetScanSession(sessionId)
    }

    /**
     * Closes and releases an active scanning session.
     */
    fun closeScanSession(sessionId: String) {
        documentCoreCloseScanSession(sessionId)
    }

    /**
     * Rectifies a perspective-distorted document quadrilateral into an upright rectangular scan frame (`SCAN-003`).
     */
    fun warpPerspective(
        frame: FfiScanFrame,
        corners: FfiQuadCorners,
        targetAspectRatio: Float? = null
    ): FfiScanFrame {
        return documentCoreWarpPerspective(frame, corners, targetAspectRatio)
    }

    /**
     * Estimates document baseline skew angle and deskews the frame (`SCAN-004`, `SCAN-005`).
     */
    fun deskewFrame(frame: FfiScanFrame): FfiDeskewResult {
        return documentCoreDeskewFrame(frame)
    }

    /**
     * Rotates a scan frame by multiples of 90 degrees clockwise (0, 90, 180, 270) (`SCAN-004`).
     */
    fun rotateFrame(frame: FfiScanFrame, clockwiseSteps: UInt): FfiScanFrame {
        return documentCoreRotateFrame(frame, clockwiseSteps)
    }

    /**
     * Automatically orients a scan frame into upright portrait based on aspect ratio (`SCAN-004`).
     */
    fun autoOrientFrame(frame: FfiScanFrame, preferPortrait: Boolean = true): FfiScanFrame {
        return documentCoreAutoOrientFrame(frame, preferPortrait)
    }

    /**
     * Retrieves the rectified warped scan page image from an active scanning session (`SCAN-003`, `SCAN-006`).
     */
    fun getWarpedScanPage(sessionId: String, pageIndex: UInt): FfiScanFrame {
        return documentCoreGetWarpedScanPage(sessionId, pageIndex)
    }

    /**
     * Enhances an ingested scan frame according to the selected cleanup profile (`SCAN-004`).
     */
    fun enhanceScanFrame(frame: FfiScanFrame, profile: FfiScanCleanupProfile): FfiScanFrame {
        return documentCoreEnhanceScanFrame(frame, profile)
    }

    /**
     * Retrieves the enhanced scan page image from an active scanning session (`SCAN-004`, `SCAN-006`).
     */
    fun getEnhancedScanPage(sessionId: String, pageIndex: UInt): FfiScanFrame {
        return documentCoreGetEnhancedScanPage(sessionId, pageIndex)
    }

    /**
     * Evaluates cleanup quality metrics between an original uncleaned frame and an enhanced frame (`SCAN-004`).
     */
    fun evaluateCleanupQuality(original: FfiScanFrame, enhanced: FfiScanFrame): FfiCleanupQualityMetrics {
        return documentCoreEvaluateCleanupQuality(original, enhanced)
    }

    /**
     * Executes optical character recognition on a raw input scan frame (`OCR-001`, `OCR-002`).
     */
    fun ocrFrame(
        frame: FfiScanFrame,
        languages: List<String> = listOf("deu", "eng"),
        detectLanguage: Boolean = true
    ): FfiOcrPageResult {
        return documentCoreOcrFrame(frame, languages, detectLanguage)
    }

    /**
     * Executes optical character recognition on a scan frame pre-processed through the
     * M1.2 intelligent scan cleanup and enhancement pipeline (`SCAN-004`).
     */
    fun ocrCleanScanFrame(
        frame: FfiScanFrame,
        profile: FfiScanCleanupProfile = FfiScanCleanupProfile.DOCUMENT_AUTO,
        languages: List<String> = listOf("deu", "eng"),
        detectLanguage: Boolean = true
    ): FfiOcrPageResult {
        return documentCoreOcrCleanScanFrame(frame, profile, languages, detectLanguage)
    }

    /**
     * Executes optical character recognition on an enhanced scan page in an active scanning session (`SCAN-006`).
     */
    fun ocrScanSessionPage(
        sessionId: String,
        pageIndex: UInt,
        languages: List<String> = listOf("deu", "eng")
    ): FfiOcrPageResult {
        return documentCoreOcrScanSessionPage(sessionId, pageIndex, languages)
    }

    /**
     * Detects primary language of a text snippet using German legal and English lexical heuristics (`OCR-002`).
     */
    fun detectTextLanguage(text: String): String {
        return documentCoreDetectTextLanguage(text)
    }

    /**
     * Synthesizes a single-page searchable PDF with an invisible OCR text layer (`PDF-001`, `OCR-004`).
     */
    fun synthesizeSearchablePdf(
        frame: FfiScanFrame,
        ocrResult: FfiOcrPageResult
    ): ByteArray {
        return documentCoreSynthesizeSearchablePdf(frame, ocrResult)
    }

    /**
     * Synthesizes a multi-page searchable PDF document directly from an active scan capture session (`PDF-001`).
     */
    fun synthesizeScanSessionPdf(
        sessionId: String,
        cleanupProfile: FfiScanCleanupProfile = FfiScanCleanupProfile.DOCUMENT_AUTO,
        languages: List<String> = listOf("deu", "eng")
    ): ByteArray {
        return documentCoreSynthesizeScanSessionPdf(sessionId, cleanupProfile, languages)
    }

    /**
     * Ingests an active scan capture session directly as an application-managed `DocumentSession` (`ROADMAP.md` M1.4).
     */
    fun ingestScanAsDocumentSession(
        scanSessionId: String,
        cleanupProfile: FfiScanCleanupProfile = FfiScanCleanupProfile.DOCUMENT_AUTO,
        languages: List<String> = listOf("deu", "eng"),
        displayName: String = ""
    ): FfiDocumentSessionSnapshot {
        return documentCoreIngestScanAsDocumentSession(scanSessionId, cleanupProfile, languages, displayName)
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
