package org.vocument.app

import uniffi.vocument_ffi.DocumentCoreHandshake
import uniffi.vocument_ffi.FfiBrokeredFileDescriptor
import uniffi.vocument_ffi.FfiDocumentSessionSnapshot
import uniffi.vocument_ffi.FfiFileAccessOrigin
import uniffi.vocument_ffi.documentCoreCloseSession
import uniffi.vocument_ffi.documentCoreGetSessionSnapshot
import uniffi.vocument_ffi.documentCoreHandshake
import uniffi.vocument_ffi.documentCoreOpenBrokeredFile

/**
 * Thin Kotlin client wrapping the generated UniFFI `DocumentCore` bindings (`ARCHITECTURE.md` §§3–6, §15).
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
}
