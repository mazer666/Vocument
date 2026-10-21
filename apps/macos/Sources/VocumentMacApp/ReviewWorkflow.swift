import SwiftUI
import AppKit
import Foundation
import VocumentMacKit
import VocumentFFI

/// The review workflow of the document (`M2`, `ANNOT-001`, `REDACT-001`,
/// `LINKSEC-001`, `ATTACH-001`, `SIGREV-001`).
///
/// The findings of the document, the comments of the reviewer with the rail
/// of the profession, and the warnings of a redaction. The warnings have to
/// reach the user: a user who sees only "the redaction succeeded" would
/// believe that the marked text is gone from the document while it is still
/// in it.
extension MacDocumentSessionCoordinator {
    /// Hands the core the directory of the data of the user.
    ///

    /// Reads the rail of the profession.
    func refreshJuristPalette() {
        juristPalette = coreClient.juristPalette()
        if !juristPalette.contains(where: { $0.key == commentPaletteKey }) {
            commentPaletteKey = juristPalette.first?.key ?? "note"
        }
    }

    /// Removes the content of the first line of the current page for good

    /// Reads the findings of the document.
    func refreshReviewFindings() {
        guard let session = activeSession else {
            reviewInspection = nil
            return
        }
        reviewInspection = try? coreClient.inspectDocument(sessionId: session.sessionId)
    }

                pendingRecovery = rec
                return
            }
        }
        do {
            if let existing = activeSession {
                try? coreClient.closeSession(sessionId: existing.sessionId)
            }
            let session = try coreClient.openScopedDocument(at: url)
            activeSession = session
            currentPageIndex = 0
            refreshReviewState()
            scaleMilli = 1000
            thumbnailImages = [:]
            searchHits = []
            exportedJournalJsonPreview = nil
            errorMessage = nil
            pageDescriptors = try coreClient.pageDescriptors(sessionId: session.sessionId)
            transactionHistory = try coreClient.transactionHistory(sessionId: session.sessionId)
            renderCurrentViewport()
            loadThumbnailsAndInitialSearchBatch(sessionId: session.sessionId)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func executeRecovery(action: FfiRecoveryAction) {
        guard let recovery = pendingRecovery else { return }
        do {
            let result = try coreClient.executeRecoveryAction(documentId: recovery.documentId, action: action)
            pendingRecovery = nil
            if let snap = result.sessionSnapshot {
                if let existing = activeSession {
                    try? coreClient.closeSession(sessionId: existing.sessionId)
                }
                activeSession = snap
                currentPageIndex = 0
                scaleMilli = 1000
                thumbnailImages = [:]
                searchHits = []
                exportedJournalJsonPreview = nil
                errorMessage = nil
                pageDescriptors = (try? coreClient.pageDescriptors(sessionId: snap.sessionId)) ?? []
                transactionHistory = try? coreClient.transactionHistory(sessionId: snap.sessionId)
                renderCurrentViewport()
                loadThumbnailsAndInitialSearchBatch(sessionId: snap.sessionId)
                saveNotificationMessage = result.message
            } else if action == .discard {
                let url = URL(fileURLWithPath: recovery.sourceToken)

    /// Reads the comments of the current page.
    func refreshPageComments() {
        guard let session = activeSession else {
            pageComments = []
            return
        }
        pageComments =
            (try? coreClient.pageCommentAnnotations(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex
            )) ?? []
    }


    var pageCount: UInt32 {
        activeSession?.pdfSummary.pageCount ?? 0
    }

    var currentLogicalLabel: String {
        pageDescriptors.first(where: { $0.pageIndex == currentPageIndex })?.logicalLabel
            ?? "\(currentPageIndex + 1)"
    }

    var prefetchedPagesSummary: String {
        prefetchedPages.isEmpty
            ? "none"
            : prefetchedPages.map { String($0 + 1) }.joined(separator: ", ")
    }

    func openDocumentViaPanel() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.pdf]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        if panel.runModal() == .OK, let url = panel.url {
            openDocument(at: url)
        }
    }

    /// Loads the findings and the comments of the document after a change of
    /// the page.
    func refreshReviewState() {
        refreshReviewFindings()
        refreshPageComments()
    }

    func openDocument(at url: URL, bypassRecoveryCheck: Bool = false) {
        if !bypassRecoveryCheck {
            if let rec = try? coreClient.checkDocumentRecovery(sourceToken: url.path) {

    /// Writes the comment the user has written on the current page.
    func addCommentToCurrentPage() {
        guard let session = activeSession else {
            return
        }
        let text = commentDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            return
        }
        let annotation = FfiCommentAnnotation(
            annotationId: "comment_\(UUID().uuidString)",
            pageIndex: currentPageIndex,
            rect: FfiPdfRectMilliPt(
                x0MilliPt: 20_000,
                y0MilliPt: 20_000,
                x1MilliPt: 36_000,
                y1MilliPt: 36_000
            ),
            paletteKey: commentPaletteKey,
            colorR: 255,
            colorG: 230,
            colorB: 80,
            contents: text,
            author: nil
        )
        do {
            _ = try coreClient.addCommentAnnotation(
                sessionId: session.sessionId,
                annotation: annotation
            )
            commentDraft = ""
            refreshPageComments()
        } catch {
            errorMessage = "\(error)"
        }
    }
}
