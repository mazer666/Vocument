package org.vocument.app

import java.io.File
import uniffi.vocument_ffi.FfiFileAccessOrigin

fun main() {
    val bridge = DocumentCoreBridge()
    val handshake = bridge.performHandshake()

    check(handshake.baselineVersion == "0.1") {
        "Expected baselineVersion '0.1', got '${handshake.baselineVersion}'"
    }
    check(handshake.milestone == "M0") {
        "Expected milestone 'M0', got '${handshake.milestone}'"
    }
    check(handshake.strictOfflineDefault) {
        "Expected strictOfflineDefault == true (NET-001)"
    }
    check(!handshake.telemetryEnabled) {
        "Expected telemetryEnabled == false (LOG-001)"
    }
    check(handshake.targetFps == 60u) {
        "Expected targetFps == 60 (PERF-001), got ${handshake.targetFps}"
    }
    check(handshake.defaultGermanVariant == "de-AT") {
        "Expected defaultGermanVariant == 'de-AT' (I18N-002), got '${handshake.defaultGermanVariant}'"
    }
    check(handshake.builtInOcrLanguages == listOf("deu", "eng")) {
        "Expected builtInOcrLanguages == ['deu', 'eng'] (OCR-002), got ${handshake.builtInOcrLanguages}"
    }

    println(
        "✓ Kotlin (Android) → UniFFI → Rust DocumentCore handshake verified: " +
            "baseline=${handshake.baselineVersion}, milestone=${handshake.milestone}, " +
            "offline=${handshake.strictOfflineDefault}, telemetry=${handshake.telemetryEnabled}, " +
            "fps=${handshake.targetFps}"
    )

    // M0.1 verification: open real PDF via Android SAF FileBroker → DocumentSession → MuPDF
    val sampleFile = File("tests/corpus/m0_1_sample_two_pages.pdf")
    val pdfBytes = sampleFile.readBytes()
    val session = bridge.openSafDocument(
        safUriString = "content://com.android.providers.downloads.documents/document/m0_1_sample",
        displayName = sampleFile.name,
        safDocumentId = "downloads:m0_1_sample",
        lastModifiedUnixMs = sampleFile.lastModified(),
        bytes = pdfBytes
    )

    check(session.sessionId.startsWith("sess_")) {
        "Expected session_id prefix 'sess_', got '${session.sessionId}'"
    }
    check(session.documentId == "doc_1a00192b75fb403b") {
        "Expected stable document_id 'doc_1a00192b75fb403b', got '${session.documentId}'"
    }
    check(session.accessOrigin == FfiFileAccessOrigin.ANDROID_SAF_URI) {
        "Expected accessOrigin == ANDROID_SAF_URI, got ${session.accessOrigin}"
    }
    check(session.fileIdentity.sizeBytes == 697uL) {
        "Expected sizeBytes == 697, got ${session.fileIdentity.sizeBytes}"
    }
    check(session.pdfSummary.pageCount == 2u) {
        "Expected pageCount == 2, got ${session.pdfSummary.pageCount}"
    }
    check(session.pdfSummary.title == "Vocument M0.1 Sample Document") {
        "Unexpected title: ${session.pdfSummary.title}"
    }
    check(session.pdfSummary.author == "Vocument Engineering") {
        "Unexpected author: ${session.pdfSummary.author}"
    }
    check(session.pdfSummary.engineIdentifier.startsWith("mupdf-1.")) {
        "Unexpected engineIdentifier: ${session.pdfSummary.engineIdentifier}"
    }

    val fetched = bridge.getSessionSnapshot(session.sessionId)
    check(fetched == session) {
        "Fetched session snapshot did not match opened session"
    }
    bridge.closeSession(session.sessionId)

    println(
        "✓ Kotlin (Android M0.1) → SAF FileBroker → DocumentSession → MuPDF verified: " +
            "session=${session.sessionId}, doc=${session.documentId}, " +
            "pages=${session.pdfSummary.pageCount}, title='${session.pdfSummary.title}', " +
            "engine=${session.pdfSummary.engineIdentifier}"
    )
}
