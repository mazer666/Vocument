import Foundation
import CoreGraphics
import VocumentFFI

/// Thin Swift-side client wrapping the UniFFI `DocumentCore` API (`ARCHITECTURE.md` §§3–10, §15).
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

    /// Forwards macOS memory-pressure events (`Normal`, `Warning`, `Critical`) into the shared adaptive `TileCache` (`ARCHITECTURE.md` §9).
    public func handleMemoryPressure(level: FfiMemoryPressureLevel) throws -> FfiTileCacheStats {
        try documentCoreHandleMemoryPressure(level: level)
    }

    /// Queries current telemetry statistics of the shared adaptive `TileCache`.
    public func tileCacheStats() throws -> FfiTileCacheStats {
        try documentCoreTileCacheStats()
    }

    /// Converts an immutable `FfiRenderedTile` RGBA8 buffer into a native `CGImage` for `DocumentCanvas` (`ARCHITECTURE.md` §10).
    public static func makeCGImage(from tile: FfiRenderedTile) -> CGImage? {
        let width = Int(tile.widthPx)
        let height = Int(tile.heightPx)
        let stride = Int(tile.strideBytes)
        guard width > 0, height > 0, tile.rgbaBytes.count >= stride * height else {
            return nil
        }
        let data = Data(tile.rgbaBytes) as CFData
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
