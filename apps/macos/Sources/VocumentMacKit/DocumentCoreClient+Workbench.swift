import Foundation
import VocumentFFI

/// Page Workbench operations extending `DocumentCoreClient` (`ROADMAP.md` M3, `ADR-0019`).
/// All mutations are verified against document invariants, execute natively in Rust/MuPDF,
/// and record reversible Undo/Redo transactions in `TransactionManager`.
extension DocumentCoreClient {
    /// Audits structural invariants, digital signatures, and encryption state
    /// before performing Page Workbench mutations (`M3.0`, `ADR-0019`).
    public func workbenchPreflight(sessionId: String) throws -> FfiPageWorkbenchPreflightReport {
        try documentCoreWorkbenchPreflight(sessionId: sessionId)
    }

    /// Reorders the pages of `sessionId` according to `order` (0-indexed permutation) (`M3.1`).
    /// Refuses if active digital signatures exist unless `allowSignatureInvalidation` is true.
    public func reorderPages(
        sessionId: String,
        order: [UInt32],
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreReorderPagesWithOptions(
            sessionId: sessionId,
            order: order,
            options: options
        )
    }

    /// Duplicates the page at `pageIndex` in `sessionId` (`M3.1`).
    /// Clones contents, annotations, and form widgets with unique deduplicated field names.
    public func duplicatePage(
        sessionId: String,
        pageIndex: UInt32,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreDuplicatePageWithOptions(
            sessionId: sessionId,
            pageIndex: pageIndex,
            options: options
        )
    }

    /// Deletes the specified `pageIndices` from `sessionId` (`M3.1`).
    /// Re-anchors surviving outline bookmarks and internal links, and cleans up form field definitions.
    public func deletePages(
        sessionId: String,
        pageIndices: [UInt32],
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreDeletePagesWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            options: options
        )
    }

    /// Extracts the selected `pageIndices` into a new standalone PDF file buffer (`M3.1`).
    /// Source document is left completely unmodified.
    public func extractPages(
        sessionId: String,
        pageIndices: [UInt32]
    ) throws -> Data {
        let rawBytes = try documentCoreExtractPages(sessionId: sessionId, pageIndices: pageIndices)
        return Data(rawBytes)
    }

    /// Rotates the specified `pageIndices` in `sessionId` by `deltaDegrees` (`M3.1`).
    /// Refuses if active digital signatures exist unless `allowSignatureInvalidation` is true.
    public func rotatePagesWithOptions(
        sessionId: String,
        pageIndices: [UInt32],
        deltaDegrees: Int32,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreRotatePagesWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            deltaDegrees: deltaDegrees,
            options: options
        )
    }

    /// Inserts pages from `sourcePdfData` at `insertAt` into `sessionId` (`M3.2`).
    /// Refuses if active digital signatures exist unless `allowSignatureInvalidation` is true.
    public func insertPages(
        sessionId: String,
        sourcePdfData: Data,
        sourcePageIndices: [UInt32],
        insertAt: UInt32,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreInsertPagesWithOptions(
            sessionId: sessionId,
            sourcePdf: sourcePdfData,
            sourcePageIndices: sourcePageIndices,
            insertAt: insertAt,
            options: options
        )
    }

    /// Merges one or more external PDF documents into `sessionId` (`M3.2`).
    /// Refuses if active digital signatures exist unless `allowSignatureInvalidation` is true.
    public func mergeDocuments(
        sessionId: String,
        sourcePdfsData: [Data],
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreMergeDocumentsWithOptions(
            sessionId: sessionId,
            sourcePdfs: sourcePdfsData,
            options: options
        )
    }

    /// Splits `sessionId` into standalone PDF byte buffers corresponding to `splitRanges` (`M3.2`).
    public func splitDocument(
        sessionId: String,
        splitRanges: [[UInt32]]
    ) throws -> [Data] {
        let resultBuffers = try documentCoreSplitDocument(
            sessionId: sessionId,
            splitRanges: splitRanges
        )
        return resultBuffers.map { Data($0) }
    }

    /// Queries the 5 standard ISO 32000-1 page boundary boxes and rotation (`CROP-002`).
    public func getPageBoxes(
        sessionId: String,
        pageIndex: UInt32
    ) throws -> FfiPdfPageBoxes {
        try documentCoreGetPageBoxes(sessionId: sessionId, pageIndex: pageIndex)
    }

    /// Sets or updates page boundary boxes (MediaBox, CropBox, BleedBox, TrimBox, ArtBox) (`CROP-002`).
    public func setPageBoxes(
        sessionId: String,
        pageIndices: [UInt32],
        boxes: FfiPageBoxUpdate,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreSetPageBoxesWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            boxes: boxes,
            options: options
        )
    }

    /// Applies a destructive hard crop with clipping path injection (`CROP-001`).
    public func applyHardCrop(
        sessionId: String,
        pageIndices: [UInt32],
        cropRect: FfiPdfRectMilliPt,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreApplyHardCropWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            cropRect: cropRect,
            options: options
        )
    }

    /// Applies a non-destructive display crop by adjusting /CropBox in the page dictionary (`CROP-001`).
    public func applyDisplayCrop(
        sessionId: String,
        pageIndices: [UInt32],
        cropRect: FfiPdfRectMilliPt,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreApplyDisplayCropWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            cropRect: cropRect,
            options: options
        )
    }

    /// Detects document skew angle on `pageIndex` in degrees (`ROTATE-003`).
    public func detectDeskewAngle(
        sessionId: String,
        pageIndex: UInt32
    ) throws -> Float {
        try documentCoreDetectDeskewAngle(sessionId: sessionId, pageIndex: pageIndex)
    }

    /// Applies fine deskew rotation via transformation matrix (`ROTATE-003`).
    public func deskewPages(
        sessionId: String,
        pageIndices: [UInt32],
        angleMilliDegrees: Int32,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreDeskewPagesWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            angleMilliDegrees: angleMilliDegrees,
            options: options
        )
    }

    /// Detects content bounding box on `pageIndex` with a safety margin (`CROP-003`).
    public func detectContentMargins(
        sessionId: String,
        pageIndex: UInt32,
        safetyMarginMilliPt: Int32
    ) throws -> FfiPdfRectMilliPt {
        try documentCoreDetectContentMargins(
            sessionId: sessionId,
            pageIndex: pageIndex,
            safetyMarginMilliPt: safetyMarginMilliPt
        )
    }

    /// Automatically detects content margins and applies a non-destructive crop with a safety margin (`CROP-003`).
    public func autoCropPages(
        sessionId: String,
        pageIndices: [UInt32],
        safetyMarginMilliPt: Int32,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let options = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreAutoCropPagesWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            safetyMarginMilliPt: safetyMarginMilliPt,
            options: options
        )
    }

    /// Applies Bates numbering to `pageIndices` in `sessionId` (`M3.4`).
    public func applyBatesNumbering(
        sessionId: String,
        pageIndices: [UInt32],
        options: FfiBatesNumberingOptions,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let workbenchOptions = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreApplyBatesNumberingWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            options: options,
            workbenchOptions: workbenchOptions
        )
    }

    /// Applies multi-zone headers and footers with dynamic macros (`M3.4`).
    public func applyHeaderFooter(
        sessionId: String,
        pageIndices: [UInt32],
        options: FfiHeaderFooterOptions,
        docTitle: String? = nil,
        dateStr: String? = nil,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let workbenchOptions = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreApplyHeaderFooterWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            options: options,
            docTitle: docTitle,
            dateStr: dateStr,
            workbenchOptions: workbenchOptions
        )
    }

    /// Retrieves document bookmark (outline) hierarchy (`M3.4`).
    public func getBookmarks(
        sessionId: String
    ) throws -> [FfiBookmarkItem] {
        try documentCoreGetBookmarks(sessionId: sessionId)
    }

    /// Replaces document bookmark (outline) hierarchy (`M3.4`).
    public func setBookmarks(
        sessionId: String,
        bookmarks: [FfiBookmarkItem],
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let workbenchOptions = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreSetBookmarksWithOptions(
            sessionId: sessionId,
            bookmarks: bookmarks,
            workbenchOptions: workbenchOptions
        )
    }

    /// Applies rubber stamp or watermark overlay to `pageIndices` (`WATER-001`–`003`, `M3.4`).
    public func applyStampOrWatermark(
        sessionId: String,
        pageIndices: [UInt32],
        options: FfiStampOrWatermarkOptions,
        allowSignatureInvalidation: Bool = false
    ) throws -> FfiTransactionRecord {
        let workbenchOptions = FfiPageWorkbenchOptions(
            allowSignatureInvalidation: allowSignatureInvalidation
        )
        return try documentCoreApplyStampOrWatermarkWithOptions(
            sessionId: sessionId,
            pageIndices: pageIndices,
            options: options,
            workbenchOptions: workbenchOptions
        )
    }
}

