import Foundation
import CoreGraphics
import VocumentFFI

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
    public func closeSession(sessionId: String) throws {
        try documentCoreCloseSession(sessionId: sessionId)
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
