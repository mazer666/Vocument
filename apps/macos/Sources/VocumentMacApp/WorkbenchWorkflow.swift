import SwiftUI
import AppKit
import Foundation
import VocumentMacKit
import VocumentFFI

/// Workflow logic and state orchestration for Visual Page Workbench (`ROADMAP.md` M3.1, `ADR-0019`).
/// Connects the native SwiftUI UI to `DocumentCoreClient` and `TransactionManager`,
/// ensuring safe transactions, multi-selection handling, preflight signature guards, and Undo/Redo.
/// Pending mutating action that requires user confirmation if signatures exist (`M3.0`, `M3.1`).
enum WorkbenchAction {
    case reorder([UInt32])
    case duplicate(UInt32)
    case delete([UInt32])
}

@MainActor
extension MacDocumentSessionCoordinator {
    // MARK: - Preflight & Invariant Auditing

    /// Audits document invariants and digital signatures before workbench actions (`M3.0`, `ADR-0019`).
    func refreshWorkbenchPreflight() {
        guard let session = activeSession else {
            workbenchPreflightReport = nil
            return
        }
        workbenchPreflightReport = try? coreClient.workbenchPreflight(sessionId: session.sessionId)
    }

    /// Performs the given workbench mutation on the active document session.
    private func executeWorkbenchAction(_ action: WorkbenchAction, allowInvalidation: Bool) throws {
        guard let session = activeSession else { return }
        switch action {
        case .reorder(let order):
            _ = try coreClient.reorderPages(
                sessionId: session.sessionId,
                order: order,
                allowSignatureInvalidation: allowInvalidation
            )
            workbenchNotificationMessage = "Pages reordered"
        case .duplicate(let pageIndex):
            _ = try coreClient.duplicatePage(
                sessionId: session.sessionId,
                pageIndex: pageIndex,
                allowSignatureInvalidation: allowInvalidation
            )
            workbenchNotificationMessage = "Page \(pageIndex + 1) duplicated"
        case .delete(let indices):
            _ = try coreClient.deletePages(
                sessionId: session.sessionId,
                pageIndices: indices,
                allowSignatureInvalidation: allowInvalidation
            )
            workbenchNotificationMessage = "Deleted \(indices.count) page(s)"
        }
    }

    /// Requests execution of `action`: if the document contains digital signatures,
    /// sets `pendingWorkbenchAction` and prompts for user confirmation; otherwise proceeds immediately.
    func requestWorkbenchAction(_ action: WorkbenchAction) {
        guard let session = activeSession else { return }
        do {
            let preflight = try coreClient.workbenchPreflight(sessionId: session.sessionId)
            workbenchPreflightReport = preflight
            if preflight.isSigned {
                pendingWorkbenchAction = action
                showSignatureConfirmation = true
            } else {
                try executeWorkbenchAction(action, allowInvalidation: false)
                refreshSessionStateAfterMutation()
            }
        } catch {
            errorMessage = "Preflight audit failed: \(error.localizedDescription)"
        }
    }

    /// User confirmed invalidating signatures: executes the pending action.
    func confirmAndExecutePendingAction() {
        guard let action = pendingWorkbenchAction else { return }
        pendingWorkbenchAction = nil
        do {
            try executeWorkbenchAction(action, allowInvalidation: true)
            refreshSessionStateAfterMutation()
        } catch {
            errorMessage = "Operation failed: \(error.localizedDescription)"
        }
    }

    /// User cancelled signature invalidation.
    func cancelPendingAction() {
        pendingWorkbenchAction = nil
    }

    // MARK: - State Refresh After Mutation

    /// Synchronizes coordinator session state, thumbnails, and descriptors after page tree mutations.
    func refreshSessionStateAfterMutation() {
        guard let session = activeSession else { return }
        do {
            let updatedSnapshot = try coreClient.sessionSnapshot(sessionId: session.sessionId)
            activeSession = updatedSnapshot
            pageDescriptors = try coreClient.pageDescriptors(sessionId: session.sessionId)
            transactionHistory = try coreClient.transactionHistory(sessionId: session.sessionId)

            // Clamp current page index to new count
            let maxPage = updatedSnapshot.pdfSummary.pageCount > 0 ? updatedSnapshot.pdfSummary.pageCount - 1 : 0
            if currentPageIndex > maxPage {
                currentPageIndex = maxPage
            }

            // Prune selection of deleted or invalid pages
            selectedWorkbenchPages = selectedWorkbenchPages.filter { $0 <= maxPage }
            if selectedWorkbenchPages.isEmpty && updatedSnapshot.pdfSummary.pageCount > 0 {
                selectedWorkbenchPages.insert(currentPageIndex)
            }

            // Re-render viewport and thumbnails
            thumbnailImages = [:]
            renderCurrentViewport()
            loadThumbnailsAndInitialSearchBatch(sessionId: session.sessionId)
            refreshReviewFindings()
            refreshWorkbenchPreflight()
            errorMessage = nil
        } catch {
            errorMessage = "Failed to synchronize session after mutation: \(error.localizedDescription)"
        }
    }

    // MARK: - Core Workbench Mutations (`M3.1`)

    /// Reorders pages according to `newOrder` permutation (`M3.1`).
    func workbenchReorderPages(newOrder: [UInt32]) {
        requestWorkbenchAction(.reorder(newOrder))
    }

    /// Moves selected pages left (-1) or right (+1) in the visual page order (`M3.1`).
    func workbenchMoveSelectedPages(delta: Int) {
        guard let session = activeSession, !selectedWorkbenchPages.isEmpty else { return }
        let count = Int(session.pdfSummary.pageCount)
        guard count > 1 else { return }

        var currentOrder = Array(0..<UInt32(count))
        let sortedSelection = selectedWorkbenchPages.sorted()

        if delta < 0 {
            // Move left: process in ascending order
            for page in sortedSelection {
                let idx = currentOrder.firstIndex(of: page) ?? -1
                if idx > 0 {
                    currentOrder.swapAt(idx, idx - 1)
                }
            }
        } else if delta > 0 {
            // Move right: process in descending order
            for page in sortedSelection.reversed() {
                let idx = currentOrder.firstIndex(of: page) ?? -1
                if idx >= 0 && idx < count - 1 {
                    currentOrder.swapAt(idx, idx + 1)
                }
            }
        }

        workbenchReorderPages(newOrder: currentOrder)
    }

    /// Duplicates the currently selected page(s) (`M3.1`).
    func workbenchDuplicateSelectedPages() {
        guard !selectedWorkbenchPages.isEmpty else { return }
        let target = selectedWorkbenchPages.min() ?? currentPageIndex
        requestWorkbenchAction(.duplicate(target))
    }

    /// Deletes the selected page(s), enforcing the last-page guard (`M3.1`).
    func workbenchDeleteSelectedPages() {
        guard let session = activeSession, !selectedWorkbenchPages.isEmpty else { return }
        if selectedWorkbenchPages.count >= Int(session.pdfSummary.pageCount) {
            errorMessage = "Cannot delete all pages of a document."
            return
        }

        let pagesToDelete = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.delete(pagesToDelete))
    }

    /// Rotates selected pages by `deltaDegrees` (e.g. 90, 180, 270) (`M3.1`, `M3.3`).
    func workbenchRotateSelectedPages(deltaDegrees: Int32) {
        guard let session = activeSession, !selectedWorkbenchPages.isEmpty else { return }
        let pagesToRotate = Array(selectedWorkbenchPages).sorted()
        do {
            transactionHistory = try coreClient.rotatePages(
                sessionId: session.sessionId,
                pageIndices: pagesToRotate,
                deltaDegrees: deltaDegrees
            )
            refreshSessionStateAfterMutation()
            workbenchNotificationMessage = "Rotated \(pagesToRotate.count) page(s) by \(deltaDegrees)°"
        } catch {
            errorMessage = "Rotation failed: \(error.localizedDescription)"
        }
    }

    /// Extracts the selected pages into a new standalone PDF document via `NSSavePanel` (`M3.1`).
    func workbenchExtractSelectedPages() {
        guard let session = activeSession, !selectedWorkbenchPages.isEmpty else { return }
        let pagesToExtract = Array(selectedWorkbenchPages).sorted()

        let panel = NSSavePanel()
        panel.allowedContentTypes = [.pdf]
        panel.canCreateDirectories = true
        panel.nameFieldStringValue = "\(session.fileIdentity.displayName.replacingOccurrences(of: ".pdf", with: ""))_extracted.pdf"

        if panel.runModal() == .OK, let targetUrl = panel.url {
            do {
                let data = try coreClient.extractPages(
                    sessionId: session.sessionId,
                    pageIndices: pagesToExtract
                )
                try data.write(to: targetUrl, options: .atomic)
                workbenchNotificationMessage = "Extracted \(pagesToExtract.count) page(s) to \(targetUrl.lastPathComponent)"
            } catch {
                errorMessage = "Extraction failed: \(error.localizedDescription)"
            }
        }
    }

    /// Undoes the last committed page mutation via `TransactionManager` (`M3.1`, `UNDO-001`).
    func workbenchUndo() {
        guard let session = activeSession else { return }
        do {
            transactionHistory = try coreClient.undo(sessionId: session.sessionId)
            refreshSessionStateAfterMutation()
            workbenchNotificationMessage = "Undo performed"
        } catch {
            errorMessage = "Undo failed: \(error.localizedDescription)"
        }
    }

    /// Redoes the last undone page mutation via `TransactionManager` (`M3.1`, `UNDO-001`).
    func workbenchRedo() {
        guard let session = activeSession else { return }
        do {
            transactionHistory = try coreClient.redo(sessionId: session.sessionId)
            refreshSessionStateAfterMutation()
            workbenchNotificationMessage = "Redo performed"
        } catch {
            errorMessage = "Redo failed: \(error.localizedDescription)"
        }
    }

    // MARK: - Multi-Selection Logic (`M3.1`)

    /// Handles single-click selection on a thumbnail.
    func workbenchSelectPage(_ index: UInt32) {
        selectedWorkbenchPages = [index]
        lastSelectedWorkbenchPageIndex = index
        currentPageIndex = index
        renderCurrentViewport()
    }

    /// Handles Cmd-Click non-contiguous selection.
    func workbenchTogglePageSelection(_ index: UInt32) {
        if selectedWorkbenchPages.contains(index) {
            selectedWorkbenchPages.remove(index)
        } else {
            selectedWorkbenchPages.insert(index)
            lastSelectedWorkbenchPageIndex = index
            currentPageIndex = index
            renderCurrentViewport()
        }
    }

    /// Handles Shift-Click contiguous range selection.
    func workbenchSelectRangeToPage(_ targetIndex: UInt32) {
        let start = lastSelectedWorkbenchPageIndex ?? currentPageIndex
        let lower = min(start, targetIndex)
        let upper = max(start, targetIndex)
        for i in lower...upper {
            selectedWorkbenchPages.insert(i)
        }
        currentPageIndex = targetIndex
        renderCurrentViewport()
    }

    /// Selects all pages in the document (`Cmd+A`).
    func workbenchSelectAllPages() {
        guard let session = activeSession else { return }
        selectedWorkbenchPages = Set(0..<session.pdfSummary.pageCount)
    }
}
