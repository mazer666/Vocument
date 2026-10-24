import SwiftUI
import AppKit
import Foundation
import VocumentMacKit
import VocumentFFI

/// Workflow orchestrations for M3.4 Bates numbering, headers/footers, bookmarks, and stamps/watermarks.
@MainActor
extension MacDocumentSessionCoordinator {
    /// Applies Bates numbering to selected pages (`M3.4`).
    func workbenchApplyBates(options: FfiBatesNumberingOptions) {
        guard !selectedWorkbenchPages.isEmpty else { return }
        let targets = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.bates(targets, options))
    }

    /// Applies multi-zone headers and footers to selected pages (`M3.4`).
    func workbenchApplyHeaderFooter(
        options: FfiHeaderFooterOptions,
        docTitle: String? = nil,
        dateStr: String? = nil
    ) {
        guard !selectedWorkbenchPages.isEmpty else { return }
        let targets = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.headerFooter(targets, options, docTitle, dateStr))
    }

    /// Loads document bookmarks/outline (`M3.4`).
    func workbenchLoadBookmarks() -> [FfiBookmarkItem] {
        guard let session = activeSession else { return [] }
        return (try? coreClient.getBookmarks(sessionId: session.sessionId)) ?? []
    }

    /// Sets document bookmarks/outline (`M3.4`).
    func workbenchSetBookmarks(bookmarks: [FfiBookmarkItem]) {
        guard activeSession != nil else { return }
        requestWorkbenchAction(.setBookmarks(bookmarks))
    }

    /// Applies rubber stamp or watermark to selected pages (`WATER-001`–`003`, `M3.4`).
    func workbenchApplyStamp(options: FfiStampOrWatermarkOptions) {
        guard !selectedWorkbenchPages.isEmpty else { return }
        let targets = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.stampOrWatermark(targets, options))
    }
}
