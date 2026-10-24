import SwiftUI
import AppKit
import Combine
import CoreGraphics
import UniformTypeIdentifiers
import VocumentMacKit
import VocumentFFI

/// Visual workflow presets for the macOS desktop application (`ROADMAP.md` M0.2, M2, M3.1).
enum MacAppViewMode: String, CaseIterable, Identifiable {
    case reader = "Reader"
    case review = "Review"
    case workbench = "Page Workbench"

    var id: String { rawValue }
}

/// Observable document session, rendering, navigation, and search view-model for macOS (`ARCHITECTURE.md` §§3–10, 17, `ROADMAP.md` M0.1–M0.3).
/// Uses `ObservableObject` so it compiles cleanly under both Xcode and standalone Command Line Tools.
@MainActor
final class MacDocumentSessionCoordinator: ObservableObject {
    /// The client of the core. The review workflow lives in the file
    /// `ReviewWorkflow.swift` and uses it as well, so it is not private to
    /// this file.
    let coreClient = DocumentCoreClient()
    private var memoryPressureSource: DispatchSourceMemoryPressure?

    @Published var activeSession: FfiDocumentSessionSnapshot?
    @Published var currentPageIndex: UInt32 = 0
    @Published var scaleMilli: UInt32 = 1000
    @Published var renderedPageImage: CGImage?
    @Published var cacheStats: FfiTileCacheStats?
    @Published var prefetchedPages: [UInt32] = []
    @Published var pageDescriptors: [FfiPageDescriptor] = []
    @Published var thumbnailImages: [UInt32: CGImage] = [:]
    @Published var navigationInput: String = ""
    @Published var searchQuery: String = ""
    @Published var searchHits: [FfiSearchHit] = []
    @Published var searchProgress: FfiSearchIndexProgress?
    @Published var selectedTextPreview: String?
    @Published var transactionHistory: FfiTransactionHistorySnapshot?
    @Published var currentPageRotationDegrees: UInt32 = 0
    @Published var currentPageHighlights: [FfiHighlightAnnotation] = []
    @Published var exportedJournalJsonPreview: String?
    @Published var activeConflict: FfiSaveConflict?
    @Published var pendingRecovery: FfiRecoverableDocumentInfo?
    @Published var saveNotificationMessage: String?
    @Published var errorMessage: String?
    /// The findings of the document: the links and the actions, the
    /// attachments and the signatures (`LINKSEC-001`, `ATTACH-001`,
    /// `SIGREV-001`).
    @Published var reviewInspection: FfiDocumentInspection?
    /// The comments of the reviewer on the current page (`ANNOT-001`).
    @Published var pageComments: [FfiCommentAnnotation] = []
    /// The rail of the profession: the colours and their meanings
    /// (`ANNOT-001`).
    @Published var juristPalette: [FfiJuristPaletteEntry] = []
    /// The text of the comment the user is writing.
    @Published var commentDraft: String = ""
    /// The key of the rail the new comment is written with.
    @Published var commentPaletteKey: String = "note"
    /// The warnings of the last redaction: the places of the document where
    /// the same text still stands, and the places that could not be read
    /// (`REDACT-002`).
    ///
    /// The warnings have to reach the user. A user who sees only "the
    /// redaction succeeded" would believe that the word is gone from the
    /// document while it is still in it.
    @Published var securityWarnings: [String] = []
    /// The rectangle the user has marked on the page (`REDACT-001`).
    ///
    /// The rectangle is in the millipoints of the page — the same space the
    /// core uses for the regions of a redaction. The user marks a **place**,
    /// not a line: a line that the application picks on its own would remove
    /// something the user never looked at.
    @Published var redactionSelection: FfiPdfRectMilliPt?
    /// The page the marked place stands on.
    ///
    /// The place belongs to the page it was marked on. A rectangle of page 3
    /// means nothing on page 5 — and a removal that did not remove the text
    /// the user saw in the preview would be a removal behind the back of the
    /// user. The redaction is therefore refused when the page has changed.
    @Published var redactionSelectionPageIndex: UInt32?
    /// The text inside the marked rectangle.
    ///
    /// The user has to see what they marked before they remove it for good.
    /// A removal that cannot be undone deserves a preview.
    @Published var redactionSelectionText: String = ""
    /// The size of the rendered page in pixels — the map from the screen to
    /// the page (`REDACT-001`).
    @Published var renderedPageSizePx: CGSize = .zero
    /// The size of the page in millipoints.
    @Published var pageSizeMilliPt: CGSize = .zero
    /// The identification of the build. The AGPL-3.0 asks every binary to
    /// name the source it was built from, so the shell shows it.
    let buildInfo: FfiBuildInfo
    /// The warning that the crash recovery of the documents is not available
    /// (`RECOVERY-001`).
    ///
    /// The recovery store holds the document of the user in plain text. It
    /// therefore only writes into a directory that belongs to the user alone.
    /// If that directory cannot be made private, the store refuses every
    /// write — and the user has to learn about it, because the documents are
    /// then not protected against a crash. A user who does not know that
    /// would believe that a crash cannot cost them anything.
    @Published var recoveryWarning: String?
    @Published var showingSanitizeConfirmation: Bool = false
    @Published var canvasDragStart: CGPoint?
    @Published var canvasDragRect: CGRect = .zero
    @Published var activeViewMode: MacAppViewMode = .workbench
    @Published var selectedWorkbenchPages: Set<UInt32> = []
    @Published var lastSelectedWorkbenchPageIndex: UInt32?
    @Published var workbenchPreflightReport: FfiPageWorkbenchPreflightReport?
    @Published var showSignatureConfirmation: Bool = false
    @Published var showDeleteConfirmation: Bool = false
    @Published var showCropSheet: Bool = false
    @Published var workbenchNotificationMessage: String?
    @Published var isWorkbenchProcessing: Bool = false
    @Published var workbenchProcessingStatus: String?
    var pendingWorkbenchAction: WorkbenchAction?

    init() {
        buildInfo = coreClient.buildInfo()
        setupSystemMemoryPressureObserver()
        configureDataDirectory()
        refreshJuristPalette()
        refreshRecoveryStatus()
    }

    /// Reads the state of the recovery store and shows the reason when the
    /// documents are not protected against a crash.
    func refreshRecoveryStatus() {
        let status = coreClient.recoveryStatus()
        if status.available {
            recoveryWarning = nil
        } else {
            recoveryWarning = status.details
        }
    }

    /// The directory holds the rail of the profession and the recovery store.
    /// The recovery store holds the document of the user in plain text and
    /// therefore belongs to the user alone: it is the application support
    /// directory of the app, not a directory of the system.
    func configureDataDirectory() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
        guard let directory = base.first else {
            return
        }
        let vocumentDirectory = directory.appendingPathComponent("Vocument", isDirectory: true)
        try? FileManager.default.createDirectory(
            at: vocumentDirectory,
            withIntermediateDirectories: true
        )
        try? coreClient.configureDataDirectory(path: vocumentDirectory.path)
    }


    /// The recovery of a document that was open when the application died
    /// (`RECOVERY-001`).
    ///
    /// The four actions of the recovery are the ones of the requirement:
    /// restore, preview, discard, open as new.
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
                // The warnings belong to the document that was open before.
                securityWarnings = []
                // The marked place belongs to the document that was open
                // before.
                clearRedactionSelection()
                pageDescriptors = (try? coreClient.pageDescriptors(sessionId: snap.sessionId)) ?? []
                transactionHistory = try? coreClient.transactionHistory(sessionId: snap.sessionId)
                // The recovered document is a document: its findings and its
                // comments have to be read, like the ones of an opened
                // document.
                refreshReviewState()
                renderCurrentViewport()
                loadThumbnailsAndInitialSearchBatch(sessionId: snap.sessionId)
                saveNotificationMessage = result.message
            } else if action == .discard {
                let url = URL(fileURLWithPath: recovery.sourceToken)
                openDocument(at: url, bypassRecoveryCheck: true)
                saveNotificationMessage = "Recovery discarded; opened original document from disk."
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    /// The number of pages of the document.
    var pageCount: UInt32 {
        activeSession?.pdfSummary.pageCount ?? 0
    }

    /// The label of the current page — the one the document gives it, not the
    /// number in the file (`PAGELABEL-002`).
    var currentLogicalLabel: String {
        pageDescriptors.first(where: { $0.pageIndex == currentPageIndex })?.logicalLabel
            ?? "\(currentPageIndex + 1)"
    }

    /// The pages that are already in the cache, for the status line.
    var prefetchedPagesSummary: String {
        prefetchedPages.isEmpty
            ? "none"
            : prefetchedPages.map { String($0 + 1) }.joined(separator: ", ")
    }

    func canStepPage(by delta: Int) -> Bool {
        guard let session = activeSession else { return false }
        let target = Int(currentPageIndex) + delta
        return target >= 0 && target < Int(session.pdfSummary.pageCount)
    }

    func stepPage(by delta: Int) {
        guard canStepPage(by: delta) else { return }
        jumpToPageIndex(UInt32(Int(currentPageIndex) + delta))
    }

    func jumpToPageIndex(_ index: UInt32) {
        guard index < pageCount else { return }
        currentPageIndex = index
        // The marked place belongs to the page it was marked on: the preview
        // of the place must not stand above another page.
        clearRedactionSelection()
        renderCurrentViewport()
        // Die Notizen und die Befunde der Seite werden mit der Seite
        // geladen: der Nutzer sieht, was auf der Seite steht.
        refreshReviewState()
    }

    func jumpToQuery() {
        guard let session = activeSession else { return }
        do {
            let resolved = try coreClient.resolvePageNavigation(
                sessionId: session.sessionId,
                query: navigationInput
            )
            jumpToPageIndex(resolved.pageIndex)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func indexNextBatch() {
        guard let session = activeSession else { return }
        do {
            searchProgress = try coreClient.indexSearchBatch(
                sessionId: session.sessionId,
                preferredStartPage: currentPageIndex,
                maxPages: 2
            )
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func performSearch() {
        guard let session = activeSession else { return }
        do {
            let response = try coreClient.searchExact(
                sessionId: session.sessionId,
                query: searchQuery
            )
            searchHits = response.hits
            searchProgress = response.progress
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func copyCurrentPageText(cleaned: Bool) {
        guard let session = activeSession else { return }
        do {
            let sel = try coreClient.selectPageText(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex,
                selectionRect: nil
            )
            let payload = cleaned ? sel.cleanedText : sel.rawText
            selectedTextPreview = payload
            let pb = NSPasteboard.general
            pb.clearContents()
            pb.setString(payload, forType: .string)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func rotateCurrentPage(deltaDegrees: Int32) {
        guard let session = activeSession else { return }
        do {
            let history = try coreClient.rotatePages(
                sessionId: session.sessionId,
                pageIndices: [currentPageIndex],
                deltaDegrees: deltaDegrees
            )
            refreshAfterMutation(sessionId: session.sessionId, history: history)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func highlightFirstLineOnCurrentPage() {
        guard let session = activeSession else { return }
        do {
            let sel = try coreClient.selectPageText(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex,
                selectionRect: nil
            )
            let rect = sel.highlightRects.first ?? FfiPdfRectMilliPt(
                x0MilliPt: 72_000,
                y0MilliPt: 700_000,
                x1MilliPt: 360_000,
                y1MilliPt: 724_000
            )
            let nextSeq = (transactionHistory?.currentRevision ?? 1)
            let annotation = FfiHighlightAnnotation(
                annotationId: "annot_mac_\(currentPageIndex)_\(nextSeq)",
                pageIndex: currentPageIndex,
                rect: rect,
                colorR: 255,
                colorG: 230,
                colorB: 80,
                opacityMilli: 450,
                author: "Vocument macOS",
                contents: "Highlighted on page #\(currentPageIndex + 1)"
            )
            let history = try coreClient.addHighlightAnnotation(
                sessionId: session.sessionId,
                annotation: annotation
            )
            refreshAfterMutation(sessionId: session.sessionId, history: history)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func removeHighlight(annotationId: String) {
        guard let session = activeSession else { return }
        do {
            let history = try coreClient.removeHighlightAnnotation(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex,
                annotationId: annotationId
            )
            refreshAfterMutation(sessionId: session.sessionId, history: history)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func performUndo() {
        guard let session = activeSession else { return }
        do {
            let history = try coreClient.undo(sessionId: session.sessionId)
            refreshAfterMutation(sessionId: session.sessionId, history: history)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func performRedo() {
        guard let session = activeSession else { return }
        do {
            let history = try coreClient.redo(sessionId: session.sessionId)
            refreshAfterMutation(sessionId: session.sessionId, history: history)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func exportPersistentJournalToClipboard() {
        guard let session = activeSession else { return }
        do {
            let json = try coreClient.exportTransactionJournalJson(sessionId: session.sessionId)
            exportedJournalJsonPreview = json
            let pb = NSPasteboard.general
            pb.clearContents()
            pb.setString(json, forType: .string)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func save() {
        guard let session = activeSession else { return }
        do {
            let outcome = try coreClient.saveSession(sessionId: session.sessionId)
            switch outcome {
            case .success(let snapshot):
                handleSaveSuccess(snapshot: snapshot)
            case .conflict(let conflict):
                activeConflict = conflict
                saveNotificationMessage = "Save conflict: File was modified externally."
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func saveAsViaPanel() {
        guard let session = activeSession else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.pdf]
        panel.nameFieldStringValue = session.fileIdentity.displayName
        if panel.runModal() == .OK, let url = panel.url {
            do {
                let outcome = try coreClient.saveSessionAs(
                    sessionId: session.sessionId,
                    targetToken: url.path
                )
                switch outcome {
                case .success(let snapshot):
                    handleSaveSuccess(snapshot: snapshot)
                case .conflict(let conflict):
                    activeConflict = conflict
                    saveNotificationMessage = "Save As conflict: Target was modified externally."
                }
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func saveCollisionFreeCopy() {
        guard let session = activeSession else { return }
        do {
            let snapshot = try coreClient.saveSessionCollisionFreeCopy(sessionId: session.sessionId)
            handleSaveSuccess(snapshot: snapshot)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func resolveActiveConflict(_ resolution: FfiSaveConflictResolution) {
        guard let session = activeSession else { return }
        activeConflict = nil
        do {
            if resolution == .reloadExternalVersion {
                _ = try coreClient.resolveSaveConflict(sessionId: session.sessionId, resolution: resolution)
                let diskUrl = URL(fileURLWithPath: session.fileIdentity.sourceToken)
                openDocument(at: diskUrl)
                saveNotificationMessage = "Reloaded external document from disk."
                return
            }
            if let snapshot = try coreClient.resolveSaveConflict(sessionId: session.sessionId, resolution: resolution) {
                handleSaveSuccess(snapshot: snapshot)
            } else {
                saveNotificationMessage = "Save cancelled."
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func handleSaveSuccess(snapshot: FfiSaveSuccessSnapshot) {
        saveNotificationMessage = "Saved \(snapshot.newFileIdentity.displayName) (Rev \(snapshot.savedRevision), \(snapshot.bytesWritten) bytes)."
        activeConflict = nil
        errorMessage = nil
        activeSession = try? coreClient.sessionSnapshot(sessionId: snapshot.sessionId)
        transactionHistory = try? coreClient.transactionHistory(sessionId: snapshot.sessionId)
    }

    /// Refreshes the picture of the document after a change of the document
    /// (`REDACT-001`).
    ///
    /// The user has to see the result of the change. A page that still shows
    /// the text that was just removed for good would be a lie: the user
    /// would believe that the text is still in the document — or that the
    /// removal never happened at all. The thumbnail of the page is refreshed
    /// as well, so that the picture in the rail of the pages does not lie
    /// either.
    func refreshRenderedPageAfterChange(sessionId: String) {
        activeSession = try? coreClient.sessionSnapshot(sessionId: sessionId)
        if let thumb = try? coreClient.renderPageThumbnail(
            sessionId: sessionId,
            pageIndex: currentPageIndex,
            maxDimensionPx: 140
        ),
           let cgImg = DocumentCoreClient.makeCGImage(from: thumb) {
            thumbnailImages[currentPageIndex] = cgImg
        }
        renderCurrentViewport()
    }

    private func refreshAfterMutation(
        sessionId: String,
        history: FfiTransactionHistorySnapshot
    ) {
        transactionHistory = history
        refreshRenderedPageAfterChange(sessionId: sessionId)
    }

    func adjustZoom(deltaMilli: Int) {
        let next = max(500, min(3000, Int(scaleMilli) + deltaMilli))
        guard UInt32(next) != scaleMilli else { return }
        scaleMilli = UInt32(next)
        renderCurrentViewport()
    }

    func applyMemoryPressure(_ level: FfiMemoryPressureLevel) {
        do {
            cacheStats = try coreClient.handleMemoryPressure(level: level)
            if level == .critical {
                thumbnailImages = [:]
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func renderCurrentViewport() {
        guard let session = activeSession, session.pdfSummary.pageCount > 0 else {
            renderedPageImage = nil
            return
        }
        do {
            let geometry = try coreClient.pageGeometry(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex
            )
            currentPageRotationDegrees = (try? coreClient.pageRotationDegrees(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex
            )) ?? 0
            currentPageHighlights = (try? coreClient.pageHighlightAnnotations(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex
            )) ?? []
            let scaledWidth = max(
                1,
                UInt32((UInt64(geometry.widthMilliPt) * UInt64(scaleMilli)) / 1_000_000)
            )
            let scaledHeight = max(
                1,
                UInt32((UInt64(geometry.heightMilliPt) * UInt64(scaleMilli)) / 1_000_000)
            )
            let request = FfiTileRenderRequest(
                pageIndex: currentPageIndex,
                scaleMilli: scaleMilli,
                tileXPx: 0,
                tileYPx: 0,
                tileWidthPx: scaledWidth,
                tileHeightPx: scaledHeight
            )
            let result = try coreClient.renderViewportWithPrefetch(
                sessionId: session.sessionId,
                request: request
            )
            renderedPageImage = DocumentCoreClient.makeCGImage(from: result.visibleTile)
            // The size of the rendered image in pixels and the size of the
            // page in millipoints are the two ends of the map from the
            // screen to the page. A marked rectangle on the screen is
            // converted with them (`REDACT-001`).
            renderedPageSizePx = CGSize(
                width: CGFloat(result.visibleTile.widthPx),
                height: CGFloat(result.visibleTile.heightPx)
            )
            pageSizeMilliPt = CGSize(
                width: CGFloat(geometry.widthMilliPt),
                height: CGFloat(geometry.heightMilliPt)
            )
            prefetchedPages = result.prefetchedPages
            cacheStats = result.cacheStats
            if let sel = try? coreClient.selectPageText(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex,
                selectionRect: nil
            ) {
                selectedTextPreview = sel.cleanedText
            }
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func loadThumbnailsAndInitialSearchBatch(sessionId: String) {
        for desc in pageDescriptors {
            if let thumb = try? coreClient.renderPageThumbnail(
                sessionId: sessionId,
                pageIndex: desc.pageIndex,
                maxDimensionPx: 140
            ),
               let cgImg = DocumentCoreClient.makeCGImage(from: thumb) {
                thumbnailImages[desc.pageIndex] = cgImg
            }
        }
        searchProgress = try? coreClient.indexSearchBatch(
            sessionId: sessionId,
            preferredStartPage: currentPageIndex,
            maxPages: 2
        )
    }

    private func setupSystemMemoryPressureObserver() {
        let source = DispatchSource.makeMemoryPressureSource(
            eventMask: [.normal, .warning, .critical],
            queue: .main
        )
        source.setEventHandler { [weak self] in
            guard let self else { return }
            let event = source.data
            if event.contains(.critical) {
                self.applyMemoryPressure(.critical)
            } else if event.contains(.warning) {
                self.applyMemoryPressure(.warning)
            } else {
                self.applyMemoryPressure(.normal)
            }
        }
        source.resume()
        self.memoryPressureSource = source
    }
}
