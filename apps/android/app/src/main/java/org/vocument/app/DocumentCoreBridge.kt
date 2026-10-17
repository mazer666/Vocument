package org.vocument.app

import uniffi.vocument_ffi.DocumentCoreHandshake
import uniffi.vocument_ffi.FfiBrokeredFileDescriptor
import uniffi.vocument_ffi.FfiDocumentSessionSnapshot
import uniffi.vocument_ffi.FfiFileAccessOrigin
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiPdfPageGeometry
import uniffi.vocument_ffi.FfiRenderedTile
import uniffi.vocument_ffi.FfiTileCacheStats
import uniffi.vocument_ffi.FfiTileRenderRequest
import uniffi.vocument_ffi.FfiViewportRenderResult
import uniffi.vocument_ffi.documentCoreCloseSession
import uniffi.vocument_ffi.documentCoreGetSessionSnapshot
import uniffi.vocument_ffi.documentCoreHandleMemoryPressure
import uniffi.vocument_ffi.documentCoreHandshake
import uniffi.vocument_ffi.documentCoreOpenBrokeredFile
import uniffi.vocument_ffi.documentCorePageGeometry
import uniffi.vocument_ffi.documentCoreRenderViewportWithPrefetch
import uniffi.vocument_ffi.documentCoreRenderVisibleTile
import uniffi.vocument_ffi.documentCoreTileCacheStats

/**
 * Thin Kotlin client wrapping the generated UniFFI `DocumentCore` bindings (`ARCHITECTURE.md` §§3–10, §15).
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
     * into the adaptive `TileCache` (`ARCHITECTURE.md` §9, `BIGMOBILE-001`).
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
     * Converts an RGBA8 `FfiRenderedTile` byte array into an `IntArray` of `ARGB_8888` pixels
     * suitable for `android.graphics.Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888)`.
     */
    fun decodeRgba8ToArgbPixels(tile: FfiRenderedTile): IntArray {
        val width = tile.widthPx.toInt()
        val height = tile.heightPx.toInt()
        val rgba = tile.rgbaBytes
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
