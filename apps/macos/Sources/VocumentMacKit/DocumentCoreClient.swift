import Foundation
import VocumentFFI

/// Thin Swift-side client wrapping the UniFFI `DocumentCore` API (`ARCHITECTURE.md` §§3–6, §15).
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
}
