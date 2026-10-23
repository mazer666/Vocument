import SwiftUI
import AppKit
import Foundation
import VocumentMacKit
import VocumentFFI

/// The workflow of the reviewer (`M2`, `ANNOT-001`, `REDACT-001`,
/// `LINKSEC-001`, `ATTACH-001`, `SIGREV-001`).
///
/// The workflow is: open a document, read the findings of the core, write the
/// comments of the reviewer with the rail of the profession, and remove what
/// has to go — for good, and with a report that says what could not be
/// removed.
///
/// The warnings have to reach the user. A user who sees only "the redaction
/// succeeded" would believe that the marked text is gone from the document
/// while it is still in it.
@MainActor
extension MacDocumentSessionCoordinator {
    // MARK: - Opening a document

    /// Lets the user choose a document (`M0.2`).
    func openDocumentViaPanel() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.pdf]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        if panel.runModal() == .OK, let url = panel.url {
            openDocument(at: url)
        }
    }

    /// Opens a document and asks the recovery store whether it knows
    /// something about it (`RECOVERY-001`).
    ///
    /// The recovery is asked **before** the document is opened: a document
    /// that was open when the application died can carry unsaved changes, and
    /// the user has to decide what happens with them.
    func openDocument(at url: URL, bypassRecoveryCheck: Bool = false) {
        if !bypassRecoveryCheck {
            if let rec = try? coreClient.checkDocumentRecovery(sourceToken: url.path) {
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
            // The warnings belong to the document that was open before.
            // A new document has not been examined yet.
            securityWarnings = []
            clearRedactionSelection()
            pageDescriptors = try coreClient.pageDescriptors(sessionId: session.sessionId)
            transactionHistory = try coreClient.transactionHistory(sessionId: session.sessionId)
            renderCurrentViewport()
            loadThumbnailsAndInitialSearchBatch(sessionId: session.sessionId)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    // MARK: - The findings and the comments of the page

    /// Reads the rail of the profession (`ANNOT-001`).
    ///
    /// The rail holds the colours and their meanings. The shell never sends a
    /// colour of its own — it sends the **key** of the rail, and the core
    /// looks the colour up.
    func refreshJuristPalette() {
        juristPalette = coreClient.juristPalette()
        if !juristPalette.contains(where: { $0.key == commentPaletteKey }) {
            commentPaletteKey = juristPalette.first?.key ?? "note"
        }
    }

    /// Reads the findings of the document: the links and the actions, the
    /// attachments and the signatures (`LINKSEC-001`, `ATTACH-001`,
    /// `SIGREV-001`).
    func refreshReviewFindings() {
        guard let session = activeSession else {
            reviewInspection = nil
            return
        }
        reviewInspection = try? coreClient.inspectDocument(sessionId: session.sessionId)
    }

    /// Reads the comments of the reviewer on the current page (`ANNOT-001`).
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

    /// Loads the findings and the comments of the document after a change of
    /// the page.
    func refreshReviewState() {
        refreshReviewFindings()
        refreshPageComments()
    }

    // MARK: - The comment of the reviewer

    /// Writes the comment the user has written on the current page
    /// (`ANNOT-001`).
    ///
    /// The note is placed at the place the user marked. The colour comes from
    /// the rail — the shell sends the key, not a colour.
    func addCommentToCurrentPage() {
        guard let session = activeSession else {
            return
        }
        let text = commentDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            return
        }
        let rect = commentPlacementRect()
        let annotation = FfiCommentAnnotation(
            annotationId: "comment_\(UUID().uuidString)",
            pageIndex: currentPageIndex,
            rect: rect,
            paletteKey: commentPaletteKey,
            // The name and the meaning of the rail are filled by the core:
            // the shell sends the key, and the core looks the rest up in the
            // rail of the user.
            paletteLabel: "",
            paletteMeaning: "",
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

    /// Takes a comment of the reviewer back (`ANNOT-001`).
    ///
    /// A comment can be taken back — unlike a redaction, which cannot.
    func removeComment(_ comment: FfiCommentAnnotation) {
        guard let session = activeSession else {
            return
        }
        do {
            _ = try coreClient.removeCommentAnnotation(
                sessionId: session.sessionId,
                pageIndex: comment.pageIndex,
                annotationId: comment.annotationId
            )
            refreshPageComments()
        } catch {
            errorMessage = "\(error)"
        }
    }

    /// The place of a new comment.
    ///
    /// The note stands where the user marked something; without a marked
    /// place it stands in the corner of the page, where the user sees it.
    private func commentPlacementRect() -> FfiPdfRectMilliPt {
        if let selection = redactionSelection {
            return selection
        }
        return FfiPdfRectMilliPt(
            x0MilliPt: 20_000,
            y0MilliPt: 20_000,
            x1MilliPt: 36_000,
            y1MilliPt: 36_000
        )
    }

    // MARK: - The redaction

    /// The chain of the redaction, carried by the kit (`REDACT-001`).
    ///
    /// The chain is in the kit and not here, so that a test can prove it —
    /// the window shows the result, the kit carries the chain.
    private var redactionReview: RedactionReview {
        RedactionReview(client: coreClient)
    }

    /// Takes the rectangle the user marked on the page and asks the core
    /// which text lies inside it (`REDACT-001`).
    ///
    /// The core decides — the shell does not guess. The text of the selection
    /// is shown to the user before the removal.
    func applyRedactionSelection(_ rect: FfiPdfRectMilliPt) {
        guard let session = activeSession else { return }
        do {
            let selection = try redactionReview.select(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex,
                rect: rect
            )
            redactionSelection = selection.rect
            // The place belongs to the page it was marked on: the removal
            // has to happen on the page the preview was taken from.
            redactionSelectionPageIndex = selection.pageIndex
            redactionSelectionText = selection.text
        } catch {
            errorMessage = "\(error)"
        }
    }

    /// Forgets the marked place.
    func clearRedactionSelection() {
        redactionSelection = nil
        redactionSelectionPageIndex = nil
        redactionSelectionText = ""
    }

    /// Removes the content of the marked place for good and verifies the
    /// removal (`REDACT-001`).
    ///
    /// The redaction cannot be undone. The report tells the shell what it has
    /// to show to the user: the words that were removed, the four checks of
    /// the verification and the places of the document where the same text
    /// still stands. The warnings go into `securityWarnings` — a user who
    /// sees only "the redaction succeeded" would believe that the word is
    /// gone from the document while it is still in it.
    func redactSelectedRegions() {
        guard let session = activeSession else { return }
        guard let rect = redactionSelection, let markedPage = redactionSelectionPageIndex else {
            errorMessage = "Mark a place on the page before you remove something."
            return
        }
        let selection = RedactionSelection(
            pageIndex: markedPage,
            rect: rect,
            text: redactionSelectionText
        )
        do {
            let outcome = try redactionReview.redact(
                sessionId: session.sessionId,
                selection: selection,
                currentPageIndex: currentPageIndex
            )
            securityWarnings = outcome.warnings
            clearRedactionSelection()
            // Clean sensitive temporary contents after successful redaction (R3-8, RISK-037):
            NSPasteboard.general.clearContents()
            thumbnailImages.removeValue(forKey: markedPage)
            refreshReviewState()
            // The user has to see the result of the removal: a page that
            // still shows the text that was just removed for good would be a
            // lie.
            refreshRenderedPageAfterChange(sessionId: session.sessionId)
        } catch let refusal as RedactionRefusal {
            // The marked place stands on another page: it is forgotten, and
            // the user has to mark it again.
            clearRedactionSelection()
            errorMessage = "\(refusal)"
            // A refused redaction changes nothing: the document is as it was.
            // There is no warning, because nothing happened.
            securityWarnings = []
        } catch {
            errorMessage = "\(error)"
            // A refused redaction changes nothing: the document is as it was.
            // There is no warning, because nothing happened.
            securityWarnings = []
        }
    }

    /// Converts a rectangle of the screen into the rectangle of the page
    /// (`REDACT-001`).
    ///
    /// The page lives in the **device space** of the page: the origin is the
    /// top left corner, the y axis points down. The rendered image has the
    /// same orientation — the first row of the image is the top of the page —
    /// so the conversion needs no flip.
    ///
    /// The two ends of the map are the size of the rendered image in pixels
    /// and the size of the page in millipoints.
    func pageRect(fromViewRect viewRect: CGRect) -> FfiPdfRectMilliPt? {
        guard renderedPageSizePx.width > 0,
              renderedPageSizePx.height > 0,
              pageSizeMilliPt.width > 0,
              pageSizeMilliPt.height > 0 else {
            return nil
        }
        let xScale = pageSizeMilliPt.width / renderedPageSizePx.width
        let yScale = pageSizeMilliPt.height / renderedPageSizePx.height
        let x0 = Int32((viewRect.minX * xScale).rounded())
        let y0 = Int32((viewRect.minY * yScale).rounded())
        let x1 = Int32((viewRect.maxX * xScale).rounded())
        let y1 = Int32((viewRect.maxY * yScale).rounded())
        return FfiPdfRectMilliPt(
            x0MilliPt: min(x0, x1),
            y0MilliPt: min(y0, y1),
            x1MilliPt: max(x0, x1),
            y1MilliPt: max(y0, y1)
        )
    }

    // MARK: - Document Sanitization (`SAN-001`)

    /// Sanitizes the active document by stripping metadata, attachments, actions, annotations, or form fields.
    func sanitizeActiveDocument(options: FfiPdfSanitizeOptions = FfiPdfSanitizeOptions(
        removeMetadata: true,
        removeAttachments: true,
        removeActions: true,
        removeAnnotations: true,
        removeFormFields: true
    )) {
        guard let session = activeSession else { return }
        do {
            _ = try coreClient.sanitizeDocument(sessionId: session.sessionId, options: options)
            refreshReviewState()
            refreshRenderedPageAfterChange(sessionId: session.sessionId)
            saveNotificationMessage = "Document sanitized: metadata, actions, and annotations cleared."
        } catch {
            errorMessage = "Sanitizing failed: \(error.localizedDescription)"
        }
    }
}
