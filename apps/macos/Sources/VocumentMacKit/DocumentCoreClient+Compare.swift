import Foundation
import VocumentFFI

/// Swift extensions for M3.7 Document Comparison & Diffing (`COMPARE-001`–`004`).
extension DocumentCoreClient {
    /// Compares two active document sessions structurally and visually.
    public func compareDocuments(
        sessionIdA: String,
        sessionIdB: String,
        options: FfiComparisonOptions = FfiComparisonOptions(
            granularity: .word,
            ignoreWhitespace: true,
            ignoreCase: false,
            visualPixelThreshold: 16,
            visualClusterDistancePt: 10.0,
            alignClauses: true
        )
    ) throws -> FfiComparisonReport {
        try documentCoreCompareDocuments(
            sessionIdA: sessionIdA,
            sessionIdB: sessionIdB,
            options: options
        )
    }

    /// Compares two specific pages visually across sessions.
    public func comparePagesVisual(
        sessionIdA: String,
        pageA: UInt32,
        sessionIdB: String,
        pageB: UInt32,
        options: FfiComparisonOptions = FfiComparisonOptions(
            granularity: .word,
            ignoreWhitespace: true,
            ignoreCase: false,
            visualPixelThreshold: 16,
            visualClusterDistancePt: 10.0,
            alignClauses: true
        )
    ) throws -> FfiPageDiffResult {
        try documentCoreComparePagesVisual(
            sessionIdA: sessionIdA,
            pageA: pageA,
            sessionIdB: sessionIdB,
            pageB: pageB,
            options: options
        )
    }

    /// Compares two raw text streams directly.
    public func compareTextStreams(
        titleA: String,
        textA: String,
        titleB: String,
        textB: String,
        options: FfiComparisonOptions = FfiComparisonOptions(
            granularity: .word,
            ignoreWhitespace: true,
            ignoreCase: false,
            visualPixelThreshold: 16,
            visualClusterDistancePt: 10.0,
            alignClauses: true
        )
    ) -> FfiComparisonReport {
        documentCoreCompareTextStreams(
            titleA: titleA,
            textA: textA,
            titleB: titleB,
            textB: textB,
            options: options
        )
    }

    /// Compares two active FlowEditor sessions.
    public func compareFlowEditorSessions(
        editorIdA: String,
        editorIdB: String,
        options: FfiComparisonOptions = FfiComparisonOptions(
            granularity: .word,
            ignoreWhitespace: true,
            ignoreCase: false,
            visualPixelThreshold: 16,
            visualClusterDistancePt: 10.0,
            alignClauses: true
        )
    ) throws -> FfiComparisonReport {
        try documentCoreCompareFlowEditorSessions(
            editorIdA: editorIdA,
            editorIdB: editorIdB,
            options: options
        )
    }
}
