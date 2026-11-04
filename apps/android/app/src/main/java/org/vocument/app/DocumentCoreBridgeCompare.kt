package org.vocument.app

import uniffi.vocument_ffi.*

/**
 * Comparison and diffing extension methods for [DocumentCoreBridge] (`M3.7`, `COMPARE-001`–`COMPARE-004`).
 */
fun DocumentCoreBridge.defaultComparisonOptions(): FfiComparisonOptions {
    return FfiComparisonOptions(
        granularity = FfiDiffGranularity.WORD,
        ignoreWhitespace = true,
        ignoreCase = false,
        visualPixelThreshold = 16.toUByte(),
        visualClusterDistancePt = 10.0f,
        alignClauses = true
    )
}

/**
 * Compares two active document sessions structurally and visually (`COMPARE-001`–`004`).
 */
fun DocumentCoreBridge.compareDocuments(
    sessionIdA: String,
    sessionIdB: String,
    options: FfiComparisonOptions = defaultComparisonOptions()
): FfiComparisonReport {
    return documentCoreCompareDocuments(sessionIdA, sessionIdB, options)
}

/**
 * Compares two specific pages visually across sessions (`COMPARE-002`).
 */
fun DocumentCoreBridge.comparePagesVisual(
    sessionIdA: String,
    pageA: UInt,
    sessionIdB: String,
    pageB: UInt,
    options: FfiComparisonOptions = defaultComparisonOptions()
): FfiPageDiffResult {
    return documentCoreComparePagesVisual(sessionIdA, pageA, sessionIdB, pageB, options)
}

/**
 * Compares two raw text streams directly (`COMPARE-001`).
 */
fun DocumentCoreBridge.compareTextStreams(
    titleA: String,
    textA: String,
    titleB: String,
    textB: String,
    options: FfiComparisonOptions = defaultComparisonOptions()
): FfiComparisonReport {
    return documentCoreCompareTextStreams(titleA, textA, titleB, textB, options)
}

/**
 * Compares two active FlowEditor sessions (`COMPARE-001`).
 */
fun DocumentCoreBridge.compareFlowEditorSessions(
    editorIdA: String,
    editorIdB: String,
    options: FfiComparisonOptions = defaultComparisonOptions()
): FfiComparisonReport {
    return documentCoreCompareFlowEditorSessions(editorIdA, editorIdB, options)
}
