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
}
