package org.vocument.app

import uniffi.vocument_ffi.*

/**
 * Page Workbench, Page Geometry, Crop and Deskew operations extending [DocumentCoreBridge]
 * (`ROADMAP.md` M3, `ADR-0019`, `CROP-001`–`CROP-003`, `ROTATE-001`–`ROTATE-003`).
 * All mutations record reversible transactions in `TransactionManager`, enforce digital signature
 * preflight guards, and execute natively in Rust/MuPDF with zero dummy mocks.
 */

/**
 * Audits structural invariants, digital signatures, and encryption state
 * before performing Page Workbench mutations (`M3.0`, `ADR-0019`).
 */
fun DocumentCoreBridge.workbenchPreflight(sessionId: String): FfiPageWorkbenchPreflightReport {
    return documentCoreWorkbenchPreflight(sessionId)
}

/**
 * Reorders the pages of `sessionId` according to `order` (0-indexed permutation) (`M3.1`).
 * Refuses if active digital signatures exist unless `allowSignatureInvalidation` is true.
 */
fun DocumentCoreBridge.reorderPages(
    sessionId: String,
    order: List<UInt>,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreReorderPagesWithOptions(sessionId, order, options)
}

/**
 * Duplicates the page at `pageIndex` in `sessionId` (`M3.1`).
 * Clones contents, annotations, and form widgets with unique deduplicated field names.
 */
fun DocumentCoreBridge.duplicatePage(
    sessionId: String,
    pageIndex: UInt,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreDuplicatePageWithOptions(sessionId, pageIndex, options)
}

/**
 * Deletes the specified `pageIndices` from `sessionId` (`M3.1`).
 * Re-anchors surviving outline bookmarks and internal links, and cleans up form field definitions.
 */
fun DocumentCoreBridge.deletePages(
    sessionId: String,
    pageIndices: List<UInt>,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreDeletePagesWithOptions(sessionId, pageIndices, options)
}

/**
 * Extracts the selected `pageIndices` into a new standalone PDF file buffer (`M3.1`).
 * Source document is left completely unmodified.
 */
fun DocumentCoreBridge.extractPages(
    sessionId: String,
    pageIndices: List<UInt>
): ByteArray {
    return documentCoreExtractPages(sessionId, pageIndices)
}

/**
 * Rotates the specified `pageIndices` in `sessionId` by `deltaDegrees` (`M3.1`).
 * Refuses if active digital signatures exist unless `allowSignatureInvalidation` is true.
 */
fun DocumentCoreBridge.rotatePages(
    sessionId: String,
    pageIndices: List<UInt>,
    deltaDegrees: Int,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreRotatePagesWithOptions(sessionId, pageIndices, deltaDegrees, options)
}

/**
 * Inserts pages from `sourcePdfData` at `insertAt` into `sessionId` (`M3.2`).
 * Refuses if active digital signatures exist unless `allowSignatureInvalidation` is true.
 */
fun DocumentCoreBridge.insertPages(
    sessionId: String,
    sourcePdfData: ByteArray,
    sourcePageIndices: List<UInt>,
    insertAt: UInt,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreInsertPagesWithOptions(sessionId, sourcePdfData, sourcePageIndices, insertAt, options)
}

/**
 * Merges one or more external PDF documents `sourcePdfsData` into `sessionId` (`M3.2`).
 * Refuses if active digital signatures exist unless `allowSignatureInvalidation` is true.
 */
fun DocumentCoreBridge.mergeDocuments(
    sessionId: String,
    sourcePdfsData: List<ByteArray>,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreMergeDocumentsWithOptions(sessionId, sourcePdfsData, options)
}

/**
 * Splits `sessionId` into standalone PDF byte buffers corresponding to `splitRanges` (`M3.2`).
 */
fun DocumentCoreBridge.splitDocument(
    sessionId: String,
    splitRanges: List<List<UInt>>
): List<ByteArray> {
    return documentCoreSplitDocument(sessionId, splitRanges)
}

/**
 * Queries the 5 standard ISO 32000-1 page boundary boxes and rotation (`CROP-002`).
 */
fun DocumentCoreBridge.getPageBoxes(
    sessionId: String,
    pageIndex: UInt
): FfiPdfPageBoxes {
    return documentCoreGetPageBoxes(sessionId, pageIndex)
}

/**
 * Sets or updates page boundary boxes (MediaBox, CropBox, BleedBox, TrimBox, ArtBox) (`CROP-002`).
 */
fun DocumentCoreBridge.setPageBoxes(
    sessionId: String,
    pageIndices: List<UInt>,
    boxes: FfiPageBoxUpdate,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreSetPageBoxesWithOptions(sessionId, pageIndices, boxes, options)
}

/**
 * Applies a destructive hard crop with clipping path injection (`CROP-001`).
 */
fun DocumentCoreBridge.applyHardCrop(
    sessionId: String,
    pageIndices: List<UInt>,
    cropRect: FfiPdfRectMilliPt,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreApplyHardCropWithOptions(sessionId, pageIndices, cropRect, options)
}

/**
 * Applies a non-destructive display crop by adjusting /CropBox in the page dictionary (`CROP-001`).
 */
fun DocumentCoreBridge.applyDisplayCrop(
    sessionId: String,
    pageIndices: List<UInt>,
    cropRect: FfiPdfRectMilliPt,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreApplyDisplayCropWithOptions(sessionId, pageIndices, cropRect, options)
}

/**
 * Detects document skew angle on `pageIndex` in degrees (`ROTATE-003`).
 */
fun DocumentCoreBridge.detectDeskewAngle(
    sessionId: String,
    pageIndex: UInt
): Float {
    return documentCoreDetectDeskewAngle(sessionId, pageIndex)
}

/**
 * Applies fine deskew rotation via transformation matrix (`ROTATE-003`).
 */
fun DocumentCoreBridge.deskewPages(
    sessionId: String,
    pageIndices: List<UInt>,
    angleMilliDegrees: Int,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreDeskewPagesWithOptions(sessionId, pageIndices, angleMilliDegrees, options)
}

/**
 * Detects content bounding box on `pageIndex` with a safety margin (`CROP-003`).
 */
fun DocumentCoreBridge.detectContentMargins(
    sessionId: String,
    pageIndex: UInt,
    safetyMarginMilliPt: Int
): FfiPdfRectMilliPt {
    return documentCoreDetectContentMargins(sessionId, pageIndex, safetyMarginMilliPt)
}

/**
 * Automatically detects content margins and applies a non-destructive crop with a safety margin (`CROP-003`).
 */
fun DocumentCoreBridge.autoCropPages(
    sessionId: String,
    pageIndices: List<UInt>,
    safetyMarginMilliPt: Int,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val options = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreAutoCropPagesWithOptions(sessionId, pageIndices, safetyMarginMilliPt, options)
}

/**
 * Applies Bates numbering to selected pages (`M3.4`).
 */
fun DocumentCoreBridge.applyBatesNumbering(
    sessionId: String,
    pageIndices: List<UInt>,
    options: FfiBatesNumberingOptions,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val workbenchOptions = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreApplyBatesNumberingWithOptions(sessionId, pageIndices, options, workbenchOptions)
}

/**
 * Applies multi-zone headers and footers with dynamic macros (`M3.4`).
 */
fun DocumentCoreBridge.applyHeaderFooter(
    sessionId: String,
    pageIndices: List<UInt>,
    options: FfiHeaderFooterOptions,
    docTitle: String? = null,
    dateStr: String? = null,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val workbenchOptions = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreApplyHeaderFooterWithOptions(sessionId, pageIndices, options, docTitle, dateStr, workbenchOptions)
}

/**
 * Retrieves document bookmark hierarchy (`M3.4`).
 */
fun DocumentCoreBridge.getBookmarks(
    sessionId: String
): List<FfiBookmarkItem> {
    return documentCoreGetBookmarks(sessionId)
}

/**
 * Replaces document bookmark hierarchy (`M3.4`).
 */
fun DocumentCoreBridge.setBookmarks(
    sessionId: String,
    bookmarks: List<FfiBookmarkItem>,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val workbenchOptions = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreSetBookmarksWithOptions(sessionId, bookmarks, workbenchOptions)
}

/**
 * Applies rubber stamp or watermark overlay to selected pages (`WATER-001`–`003`, `M3.4`).
 */
fun DocumentCoreBridge.applyStampOrWatermark(
    sessionId: String,
    pageIndices: List<UInt>,
    options: FfiStampOrWatermarkOptions,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val workbenchOptions = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreApplyStampOrWatermarkWithOptions(sessionId, pageIndices, options, workbenchOptions)
}

/**
 * Optimizes document streams, compacts xref into object streams, and removes duplicates (`M3.5`).
 */
fun DocumentCoreBridge.optimizeDocument(
    sessionId: String,
    options: FfiOptimizationOptions,
    allowSignatureInvalidation: Boolean = false
): FfiTransactionRecord {
    val workbenchOptions = FfiPageWorkbenchOptions(allowSignatureInvalidation = allowSignatureInvalidation)
    return documentCoreOptimizeDocumentWithOptions(sessionId, options, workbenchOptions)
}

/**
 * Discovers embedded raster images across the document (`M3.5`).
 */
fun DocumentCoreBridge.getDocumentImages(
    sessionId: String
): List<FfiExtractedImageInfo> {
    return documentCoreGetDocumentImages(sessionId)
}

/**
 * Extracts raw lossless stream bytes (JPEG/JP2) or rendered PNG bytes from a page (`M3.5`).
 */
fun DocumentCoreBridge.extractImageBytes(
    sessionId: String,
    pageIndex: UInt,
    imageIndex: UInt
): FfiExtractedImageData {
    return documentCoreExtractImageBytes(sessionId, pageIndex, imageIndex)
}

/**
 * Performs restricted structural preflight against ISO 15930 PDF/X rules (`M3.5`).
 */
fun DocumentCoreBridge.preflightPdfx(
    sessionId: String
): FfiPdfxPreflightReport {
    return documentCorePreflightPdfx(sessionId)
}

/**
 * Preflights document dimensions and margin boxes for professional printing (`PRINT-003`, `PRINT-004`).
 */
fun DocumentCoreBridge.preflightPrint(
    sessionId: String
): FfiPrintPreflightReport {
    return documentCorePreflightPrint(sessionId)
}


