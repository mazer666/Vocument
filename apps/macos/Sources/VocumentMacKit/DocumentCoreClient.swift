import Foundation
import CoreGraphics
import VocumentFFI

/// The errors of the Swift-side client.
///
/// The client reports what the core could not promise: a redaction whose
/// verification did not succeed in every respect. The core has already
/// refused such a redaction — the check here is the second line, in case the
/// boundary ever changes. The error of the boundary itself is thrown, so that
/// a shell that catches the errors of the core catches this one as well: a
/// shell that only knows its own error type would swallow the refusal and
/// show a refused redaction as a successful one.
public enum DocumentCoreClientError: Error, CustomStringConvertible {
    /// The redaction could not be verified in every respect.
    case redactionNotVerified(String)

    public var description: String {
        switch self {
        case .redactionNotVerified(let details):
            return details
        }
    }
}

/// Thin Swift-side client wrapping the UniFFI `DocumentCore` API (`ARCHITECTURE.md` §§3–10, 15, 17, `ROADMAP.md` M0.0–M0.3).
/// Never hardcodes product defaults or accesses MuPDF directly; all document state is owned by `vocument-core`.
public struct DocumentCoreClient: Sendable {
    public init() {}

    /// Invokes the Rust core across UniFFI and returns the verified bootstrap handshake snapshot.
    public func performHandshake() throws -> DocumentCoreHandshake {
        try documentCoreHandshake()
    }

    /// Opens a PDF document selected via Finder / Open With security-scoped URL on macOS (`ROADMAP.md` M0.1).
    public func openScopedDocument(at url: URL) throws -> FfiDocumentSessionSnapshot {
        let didStartAccessing = url.startAccessingSecurityScopedResource()
        defer {
            if didStartAccessing {
                url.stopAccessingSecurityScopedResource()
            }
        }
        return try documentCoreOpenMacosScopedFile(scopedPathOrUrl: url.path)
    }

    /// Opens a PDF document from a local file path via `ScopedLocalFileBroker`.
    public func openScopedFilePath(_ path: String) throws -> FfiDocumentSessionSnapshot {
        try documentCoreOpenMacosScopedFile(scopedPathOrUrl: path)
    }

    /// Retrieves the latest immutable snapshot of an active `DocumentSession`.
    public func sessionSnapshot(sessionId: String) throws -> FfiDocumentSessionSnapshot {
        try documentCoreGetSessionSnapshot(sessionId: sessionId)
    }

    /// Closes an active `DocumentSession` and releases its engine resources.
    ///
    /// Every background job of the session is cancelled by the core before the
    /// session is destroyed, so a late result can never be applied.
    public func closeSession(sessionId: String) throws {
        try documentCoreCloseSession(sessionId: sessionId)
    }

    // MARK: - Background job manager (`ARCHITECTURE.md` §8, `ROADMAP.md` M1.5)

    /// Submits background work for a document.
    ///
    /// The work runs on the bounded worker pool inside the Rust core. This call
    /// returns immediately; progress is polled with `jobStatus(jobId:)`.
    public func submitJob(
        sessionId: String,
        documentRevision: UInt64,
        kind: String,
        resumable: Bool = false
    ) throws -> UInt64 {
        try documentCoreJobSubmit(
            sessionId: sessionId,
            documentRevision: documentRevision,
            kind: kind,
            resumable: resumable
        )
    }

    /// Returns the current status of a background job.
    public func jobStatus(jobId: UInt64) throws -> FfiJobStatus {
        try documentCoreJobStatus(jobId: jobId)
    }

    /// Returns the status of every known background job.
    public func jobStatuses() throws -> [FfiJobStatus] {
        try documentCoreJobStatuses()
    }

    /// Requests a cooperative cancellation of a background job.
    public func cancelJob(jobId: UInt64) throws -> String {
        try documentCoreJobCancel(jobId: jobId)
    }

    /// Cancels every background job of a session.
    public func cancelJobs(sessionId: String) throws -> UInt32 {
        try documentCoreJobCancelSession(sessionId: sessionId)
    }

    /// Returns the number of jobs waiting in the queue.
    public func queuedJobCount() throws -> UInt32 {
        try documentCoreJobQueuedCount()
    }

    /// Returns the number of jobs currently executing.
    public func runningJobCount() throws -> UInt32 {
        try documentCoreJobRunningCount()
    }

    /// Queries the point geometry (`widthMilliPt`, `heightMilliPt`) of `pageIndex` via `DocumentActor` (`ROADMAP.md` M0.2).
    public func pageGeometry(sessionId: String, pageIndex: UInt32) throws -> FfiPdfPageGeometry {
        try documentCorePageGeometry(sessionId: sessionId, pageIndex: pageIndex)
    }

    /// Renders a visible tile at `VisiblePageRender` priority via `DocumentActor` and adaptive `TileCache` (`ROADMAP.md` M0.2).
    public func renderVisibleTile(
        sessionId: String,
        request: FfiTileRenderRequest
    ) throws -> FfiRenderedTile {
        try documentCoreRenderVisibleTile(sessionId: sessionId, request: request)
    }

    /// Renders the requested visible viewport tile and prefetches neighbouring pages (`ROADMAP.md` M0.2).
    public func renderViewportWithPrefetch(
        sessionId: String,
        request: FfiTileRenderRequest
    ) throws -> FfiViewportRenderResult {
        try documentCoreRenderViewportWithPrefetch(sessionId: sessionId, request: request)
    }

    /// Forwards macOS memory-pressure events (`Normal`, `Warning`, `Critical`) into the shared adaptive caches (`ARCHITECTURE.md` §9).
    public func handleMemoryPressure(level: FfiMemoryPressureLevel) throws -> FfiTileCacheStats {
        try documentCoreHandleMemoryPressure(level: level)
    }

    /// Queries current telemetry statistics of the shared adaptive `TileCache`.
    public func tileCacheStats() throws -> FfiTileCacheStats {
        try documentCoreTileCacheStats()
    }

    /// Returns all physical and logical `FfiPageDescriptor` entries for `sessionId` (`PAGELABEL-001`, `PAGELABEL-002`).
    public func pageDescriptors(sessionId: String) throws -> [FfiPageDescriptor] {
        try documentCorePageDescriptors(sessionId: sessionId)
    }

    /// Resolves a user navigation target (`query`: logical label like `"ii"` or `"1"`, or physical page `"#2"`) (`PAGELABEL-003`).
    public func resolvePageNavigation(
        sessionId: String,
        query: String
    ) throws -> FfiPageDescriptor {
        try documentCoreResolvePageNavigation(sessionId: sessionId, query: query)
    }

    /// Creates or replaces custom logical page-label ranges on `sessionId` (`PAGELABEL-004`).
    public func setPageLabelRanges(
        sessionId: String,
        ranges: [FfiPageLabelRange]
    ) throws -> [FfiPageDescriptor] {
        try documentCoreSetPageLabelRanges(sessionId: sessionId, ranges: ranges)
    }

    /// Renders a low-resolution page thumbnail fitted within `maxDimensionPx` via the separate `ThumbnailCache` (`PAGEVIEW-001`, `PAGEVIEW-002`).
    public func renderPageThumbnail(
        sessionId: String,
        pageIndex: UInt32,
        maxDimensionPx: UInt32
    ) throws -> FfiRenderedThumbnail {
        try documentCoreRenderPageThumbnail(
            sessionId: sessionId,
            pageIndex: pageIndex,
            maxDimensionPx: maxDimensionPx
        )
    }

    /// Queries current telemetry statistics of the separate `ThumbnailCache`.
    public func thumbnailCacheStats() throws -> FfiThumbnailCacheStats {
        try documentCoreThumbnailCacheStats()
    }

    /// Extracts and selects text on `pageIndex` (full-page when `selectionRect` is `nil`, or intersecting `selectionRect`),
    /// returning both `rawText` (`OCRCOPY-001`) and `cleanedText` (`OCRCOPY-002`).
    public func selectPageText(
        sessionId: String,
        pageIndex: UInt32,
        selectionRect: FfiPdfRectMilliPt? = nil
    ) throws -> FfiTextSelectionResult {
        try documentCoreSelectPageText(
            sessionId: sessionId,
            pageIndex: pageIndex,
            selectionRect: selectionRect
        )
    }

    /// Progressively indexes up to `maxPages` into the session's SQLite FTS5 index starting around `preferredStartPage` (`SEARCH-009`).
    public func indexSearchBatch(
        sessionId: String,
        preferredStartPage: UInt32,
        maxPages: UInt32
    ) throws -> FfiSearchIndexProgress {
        try documentCoreIndexSearchBatch(
            sessionId: sessionId,
            preferredStartPage: preferredStartPage,
            maxPages: maxPages
        )
    }

    /// Queries the current progressive SQLite FTS5 indexing progress for `sessionId`.
    public func searchIndexProgress(sessionId: String) throws -> FfiSearchIndexProgress {
        try documentCoreSearchIndexProgress(sessionId: sessionId)
    }

    /// Executes a classic exact FTS5 search against all currently indexed pages in `sessionId` (`SEARCH-002`).
    public func searchExact(sessionId: String, query: String) throws -> FfiSearchResponse {
        try documentCoreSearchExact(sessionId: sessionId, query: query)
    }

    /// Queries the current rotation angle (`0`, `90`, `180`, `270`) of `pageIndex` (`ROTATE-001`, `ROADMAP.md` M0.4).
    public func pageRotationDegrees(sessionId: String, pageIndex: UInt32) throws -> UInt32 {
        try documentCorePageRotationDegrees(sessionId: sessionId, pageIndex: pageIndex)
    }

    /// Lists all standard PDF `/Highlight` annotations on `pageIndex` (`ANNOT-001`, `INTEROP-002`).
    public func pageHighlightAnnotations(
        sessionId: String,
        pageIndex: UInt32
    ) throws -> [FfiHighlightAnnotation] {
        try documentCorePageHighlightAnnotations(sessionId: sessionId, pageIndex: pageIndex)
    }

    /// Rotates one or more pages by `deltaDegrees` (`+90`, `-90`, `180`) via `TransactionManager` (`ROTATE-001`, `PAGE-001`, `ROADMAP.md` M0.4 Gate).
    public func rotatePages(
        sessionId: String,
        pageIndices: [UInt32],
        deltaDegrees: Int32
    ) throws -> FfiTransactionHistorySnapshot {
        try documentCoreRotatePages(
            sessionId: sessionId,
            pageIndices: pageIndices,
            deltaDegrees: deltaDegrees
        )
    }

    /// Adds a standard PDF `/Highlight` annotation via `TransactionManager` (`ANNOT-001`, `INTEROP-002`, `ROADMAP.md` M0.4 Gate).
    public func addHighlightAnnotation(
        sessionId: String,
        annotation: FfiHighlightAnnotation
    ) throws -> FfiTransactionHistorySnapshot {
        try documentCoreAddHighlightAnnotation(sessionId: sessionId, annotation: annotation)
    }

    /// Removes a standard PDF `/Highlight` annotation by `annotationId` via `TransactionManager` (`ANNOT-001`).
    public func removeHighlightAnnotation(
        sessionId: String,
        pageIndex: UInt32,
        annotationId: String
    ) throws -> FfiTransactionHistorySnapshot {
        try documentCoreRemoveHighlightAnnotation(
            sessionId: sessionId,
            pageIndex: pageIndex,
            annotationId: annotationId
        )
    }

    /// The rail of the profession: the palette the comments are written with
    /// (`ANNOT-001`).
    ///
    /// The shell does not know the colours of the notes — it chooses the key
    /// of the rail, and the core looks up the colour.
    public func juristPalette() -> [FfiJuristPaletteEntry] {
        documentCoreJuristPalette()
    }

    /// Replaces the rail of the profession with the palette of the user
    /// (`ANNOT-001`).
    public func setJuristPalette(entries: [FfiJuristPaletteEntry]) throws {
        try documentCoreSetJuristPalette(entries: entries)
    }

    /// Lists the comments of the reviewer on `pageIndex` (`ANNOT-001`).
    public func pageCommentAnnotations(
        sessionId: String,
        pageIndex: UInt32
    ) throws -> [FfiCommentAnnotation] {
        try documentCorePageCommentAnnotations(sessionId: sessionId, pageIndex: pageIndex)
    }

    /// Writes a comment of the reviewer via `TransactionManager` (`ANNOT-001`).
    ///
    /// The core refuses a note without a text and a key that the rail does
    /// not carry; the document stays untouched when the note is refused.
    public func addCommentAnnotation(
        sessionId: String,
        annotation: FfiCommentAnnotation
    ) throws -> FfiTransactionHistorySnapshot {
        try documentCoreAddCommentAnnotation(sessionId: sessionId, annotation: annotation)
    }

    /// Removes a comment of the reviewer by `annotationId` via `TransactionManager` (`ANNOT-001`).
    public func removeCommentAnnotation(
        sessionId: String,
        pageIndex: UInt32,
        annotationId: String
    ) throws -> FfiTransactionHistorySnapshot {
        try documentCoreRemoveCommentAnnotation(
            sessionId: sessionId,
            pageIndex: pageIndex,
            annotationId: annotationId
        )
    }

    /// Inspects the links and the actions, the attachments and the signatures
    /// of a document (`LINKSEC-001`, `ATTACH-001`, `SIGREV-001`).
    ///
    /// The report tells the shell where a document tries to do something the
    /// user should know about: a script, a link that leaves the document, a
    /// form that sends data away, an embedded file, a signature that the
    /// document changed after it was signed.
    public func inspectDocument(sessionId: String) throws -> FfiDocumentInspection {
        try documentCoreInspectDocument(sessionId: sessionId)
    }

    /// Removes the content of the marked regions of a page for good
    /// (`REDACT-001`, `REDACT-002`).
    ///
    /// The redaction cannot be undone. The report tells the shell what it has
    /// to show to the user before the document is saved: the words that were
    /// removed, the four checks of the verification and the places of the
    /// document where the same word still stands.
    ///
    /// The client does not trust the `verified` flag of the report: it looks
    /// at the four checks themselves. A check that was not carried out
    /// (`notChecked`) is not a pass.
    public func redactPageRegions(
        sessionId: String,
        pageIndex: UInt32,
        regions: [FfiRedactionRegion]
    ) throws -> FfiRedactionReport {
        let report = try documentCoreRedactPageRegions(
            sessionId: sessionId,
            pageIndex: pageIndex,
            regions: regions
        )
        let checks: [(String, FfiRedactionCheck)] = [
            ("the text layer of the page", report.textLayerClean),
            ("the search over the document", report.searchIndexClean),
            ("the picture of the page", report.renderedPageClean),
            ("the bytes of the file", report.fileBytesClean),
        ]
        for (what, check) in checks where check != .passed {
            // The error of the boundary itself: a shell that catches the
            // errors of the core catches this one as well, and it cannot
            // mistake a refused redaction for a successful one.
            throw DocumentCoreError.RedactionUnverified(
                details: "the redaction of the page \(pageIndex) could not be verified: \(what) is not clean"
            )
        }
        return report
    }

    /// The warnings of a redaction: the places of the document where the same
    /// text still stands, and the places that could not be read.
    ///
    /// The redaction removes what the user marked. The same word can stand
    /// somewhere else in the document — on another page, in another place of
    /// the same page — and a place that cannot be read at all is not a proof
    /// that the text is gone. The shell **has to** show these warnings: a
    /// user who is told only that the redaction succeeded would believe that
    /// the word is gone from the document, while it is still in it.
    public static func redactionWarnings(_ report: FfiRedactionReport) -> [String] {
        var warnings: [String] = []
        for place in report.remainingOccurrences {
            warnings.append(
                "the same text still stands at \(place.location) (page \(place.pageIndex))"
            )
        }
        for place in report.uninspectedPlaces {
            warnings.append(
                "this place could not be read: \(place.location) (page \(place.pageIndex))"
            )
        }
        return warnings
    }

    /// Undoes the last committed transaction on `sessionId` via `TransactionManager` (`UNDO-001`, `UNDO-004`).
    public func undo(sessionId: String) throws -> FfiTransactionHistorySnapshot {
        try documentCoreUndo(sessionId: sessionId)
    }

    /// Redoes the last undone transaction on `sessionId` via `TransactionManager` (`UNDO-001`, `UNDO-004`).
    public func redo(sessionId: String) throws -> FfiTransactionHistorySnapshot {
        try documentCoreRedo(sessionId: sessionId)
    }

    /// Queries the live Undo/Redo and transaction history state for `sessionId` (`UNDO-001`, `UNDO-002`).
    public func transactionHistory(sessionId: String) throws -> FfiTransactionHistorySnapshot {
        try documentCoreTransactionHistory(sessionId: sessionId)
    }

    /// Exports the verified `PersistentTransactionJournal` as pretty-printed JSON (`ARCHITECTURE.md` §§11–12).
    public func exportTransactionJournalJson(sessionId: String) throws -> String {
        try documentCoreExportTransactionJournalJson(sessionId: sessionId)
    }

    /// Restores and replays a `PersistentTransactionJournal` JSON payload onto `sessionId` (`UNDO-001`).
    public func restoreTransactionJournalJson(
        sessionId: String,
        journalJson: String
    ) throws -> FfiTransactionHistorySnapshot {
        try documentCoreRestoreTransactionJournalJson(
            sessionId: sessionId,
            journalJson: journalJson
        )
    }

    /// Safe save of active `sessionId` back to its original file (`ROADMAP.md` M0.5, `SAVE-001`).
    public func saveSession(sessionId: String) throws -> FfiSaveOutcome {
        try documentCoreSaveSession(sessionId: sessionId)
    }

    /// 'Save As' of active `sessionId` to `targetToken` (`SAVE-002`).
    public func saveSessionAs(sessionId: String, targetToken: String) throws -> FfiSaveOutcome {
        try documentCoreSaveSessionAs(sessionId: sessionId, targetToken: targetToken)
    }

    /// Saves a collision-free safe copy of `sessionId` (`ARCHITECTURE.md` §16, `FILELOCK-004`).
    public func saveSessionCollisionFreeCopy(sessionId: String) throws -> FfiSaveSuccessSnapshot {
        try documentCoreSaveSessionCollisionFreeCopy(sessionId: sessionId)
    }

    /// Inspects whether on-disk file for `sessionId` was modified externally (`FILELOCK-002`).
    public func checkExternalChanges(sessionId: String) throws -> FfiExternalChangeCheckResult {
        try documentCoreCheckExternalChanges(sessionId: sessionId)
    }

    /// Resolves an external save conflict according to `resolution` (`ARCHITECTURE.md` §16, `FILELOCK-004`).
    public func resolveSaveConflict(
        sessionId: String,
        resolution: FfiSaveConflictResolution
    ) throws -> FfiSaveSuccessSnapshot? {
        try documentCoreResolveSaveConflict(sessionId: sessionId, resolution: resolution)
    }

    /// Generates a collision-free file path for `basePath` and `suffix` (`FILELOCK-004`).
    public func generateCollisionFreePath(basePath: String, suffix: String) -> String {
        documentCoreGenerateCollisionFreePath(basePath: basePath, suffix: suffix)
    }

    // MARK: - M0.6 Recovery APIs

    /// Scans the recovery store and returns recoverable documents that experienced an unclean shutdown (`ROADMAP.md` M0.6).
    public func scanRecoverableDocuments() throws -> [FfiRecoverableDocumentInfo] {
        try documentCoreScanRecoverableDocuments()
    }

    /// Checks whether a specific document has a pending crash recovery container (`ROADMAP.md` M0.6).
    public func checkDocumentRecovery(sourceToken: String) throws -> FfiRecoverableDocumentInfo? {
        try documentCoreCheckDocumentRecovery(sourceToken: sourceToken)
    }

    /// Executes a recovery action (`.restore`, `.preview`, `.discard`, `.openAsNew`) on `documentId` (`RECOVERY-001`).
    ///
    /// **M0.6 Gate Invariant:** Disk file remains completely intact until explicit user save.
    public func executeRecoveryAction(
        documentId: String,
        action: FfiRecoveryAction
    ) throws -> FfiRecoveryActionResult {
        try documentCoreExecuteRecoveryAction(documentId: documentId, action: action)
    }

    /// Marks a document's recovery container clean shutdown.
    public func markRecoveryCleanShutdown(documentId: String) throws {
        try documentCoreMarkRecoveryCleanShutdown(documentId: documentId)
    }

    /// Executes the full M0 DoD workflow and verifies generated PDF files across reference and foreign viewers (`ROADMAP.md` M0.7, `M0-ACCEPTANCE.md`).
    public func runInteropGateSuite() throws -> FfiInteropReport {
        try documentCoreRunInteropGateSuite()
    }

    /// Converts an immutable `FfiRenderedTile` RGBA8 buffer into a native `CGImage` (`ARCHITECTURE.md` §10).
    public static func makeCGImage(from tile: FfiRenderedTile) -> CGImage? {
        makeCGImageFromRgba8(
            width: Int(tile.widthPx),
            height: Int(tile.heightPx),
            stride: Int(tile.strideBytes),
            rgbaBytes: tile.rgbaBytes
        )
    }

    /// Converts an immutable `FfiRenderedThumbnail` RGBA8 buffer into a native `CGImage`.
    public static func makeCGImage(from thumbnail: FfiRenderedThumbnail) -> CGImage? {
        makeCGImageFromRgba8(
            width: Int(thumbnail.widthPx),
            height: Int(thumbnail.heightPx),
            stride: Int(thumbnail.strideBytes),
            rgbaBytes: thumbnail.rgbaBytes
        )
    }

    private static func makeCGImageFromRgba8(
        width: Int,
        height: Int,
        stride: Int,
        rgbaBytes: Data
    ) -> CGImage? {
        guard width > 0, height > 0, rgbaBytes.count >= stride * height else {
            return nil
        }
        let data = rgbaBytes as CFData
        guard let provider = CGDataProvider(data: data) else {
            return nil
        }
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue)
        return CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: stride,
            space: colorSpace,
            bitmapInfo: bitmapInfo,
            provider: provider,
            decode: nil,
            shouldInterpolate: true,
            intent: .defaultIntent
        )
    }
}
