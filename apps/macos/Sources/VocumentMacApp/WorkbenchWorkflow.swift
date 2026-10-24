import SwiftUI
import AppKit
import Foundation
import VocumentMacKit
import VocumentFFI

/// Workflow logic and state orchestration for Visual Page Workbench (`ROADMAP.md` M3.1, `ADR-0019`).
/// Connects the native SwiftUI UI to `DocumentCoreClient` and `TransactionManager`,
/// ensuring safe transactions, multi-selection handling, preflight signature guards, and Undo/Redo.
/// Pending mutating action that requires user confirmation if signatures exist (`M3.0`, `M3.1`, `M3.2`, `M3.3`).
enum WorkbenchAction {
    case reorder([UInt32])
    case duplicate([UInt32])
    case delete([UInt32])
    case rotate([UInt32], Int32)
    case insert(Data, [UInt32], UInt32)
    case merge([Data])
    case hardCrop([UInt32], FfiPdfRectMilliPt)
    case displayCrop([UInt32], FfiPdfRectMilliPt)
    case deskew([UInt32], Int32)
    case autoCrop([UInt32], Int32)
    case bates([UInt32], FfiBatesNumberingOptions)
    case headerFooter([UInt32], FfiHeaderFooterOptions, String?, String?)
    case setBookmarks([FfiBookmarkItem])
    case stampOrWatermark([UInt32], FfiStampOrWatermarkOptions)
    case optimize(FfiOptimizationOptions)
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
    private func executeWorkbenchAction(_ action: WorkbenchAction, allowInvalidation: Bool) throws -> String {
        guard let session = activeSession else { return "" }
        switch action {
        case .reorder(let order):
            _ = try coreClient.reorderPages(
                sessionId: session.sessionId,
                order: order,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Pages reordered"
        case .duplicate(let pageIndices):
            for pageIndex in pageIndices.sorted().reversed() {
                _ = try coreClient.duplicatePage(
                    sessionId: session.sessionId,
                    pageIndex: pageIndex,
                    allowSignatureInvalidation: allowInvalidation
                )
            }
            return "Duplicated \(pageIndices.count) page(s)"
        case .delete(let indices):
            _ = try coreClient.deletePages(
                sessionId: session.sessionId,
                pageIndices: indices,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Deleted \(indices.count) page(s)"
        case .rotate(let indices, let delta):
            _ = try coreClient.rotatePagesWithOptions(
                sessionId: session.sessionId,
                pageIndices: indices,
                deltaDegrees: delta,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Rotated \(indices.count) page(s) by \(delta)°"
        case .insert(let sourceData, let sourceIndices, let insertAt):
            _ = try coreClient.insertPages(
                sessionId: session.sessionId,
                sourcePdfData: sourceData,
                sourcePageIndices: sourceIndices,
                insertAt: insertAt,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Inserted \(sourceIndices.count) page(s) at position #\(insertAt + 1)"
        case .merge(let sourcePdfsData):
            _ = try coreClient.mergeDocuments(
                sessionId: session.sessionId,
                sourcePdfsData: sourcePdfsData,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Merged \(sourcePdfsData.count) external document(s)"
        case .hardCrop(let indices, let rect):
            _ = try coreClient.applyHardCrop(
                sessionId: session.sessionId,
                pageIndices: indices,
                cropRect: rect,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Applied hard crop to \(indices.count) page(s)"
        case .displayCrop(let indices, let rect):
            _ = try coreClient.applyDisplayCrop(
                sessionId: session.sessionId,
                pageIndices: indices,
                cropRect: rect,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Applied display crop to \(indices.count) page(s)"
        case .deskew(let indices, let angleMilliDegrees):
            _ = try coreClient.deskewPages(
                sessionId: session.sessionId,
                pageIndices: indices,
                angleMilliDegrees: angleMilliDegrees,
                allowSignatureInvalidation: allowInvalidation
            )
            let angleDeg = Double(angleMilliDegrees) / 1000.0
            return String(format: "Deskewed %d page(s) by %.2f°", indices.count, angleDeg)
        case .autoCrop(let indices, let safetyMargin):
            _ = try coreClient.autoCropPages(
                sessionId: session.sessionId,
                pageIndices: indices,
                safetyMarginMilliPt: safetyMargin,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Auto-cropped margins on \(indices.count) page(s)"
        case .bates(let indices, let options):
            _ = try coreClient.applyBatesNumbering(
                sessionId: session.sessionId,
                pageIndices: indices,
                options: options,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Applied Bates numbering to \(indices.count) page(s)"
        case .headerFooter(let indices, let options, let title, let date):
            _ = try coreClient.applyHeaderFooter(
                sessionId: session.sessionId,
                pageIndices: indices,
                options: options,
                docTitle: title,
                dateStr: date,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Applied headers/footers to \(indices.count) page(s)"
        case .setBookmarks(let bookmarks):
            _ = try coreClient.setBookmarks(
                sessionId: session.sessionId,
                bookmarks: bookmarks,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Updated document bookmarks (\(bookmarks.count) root items)"
        case .stampOrWatermark(let indices, let options):
            _ = try coreClient.applyStampOrWatermark(
                sessionId: session.sessionId,
                pageIndices: indices,
                options: options,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Applied '\(options.text)' stamp to \(indices.count) page(s)"
        case .optimize(let options):
            _ = try coreClient.optimizeDocument(
                sessionId: session.sessionId,
                options: options,
                allowSignatureInvalidation: allowInvalidation
            )
            return "Optimized document streams and structure"
        }
    }

    /// Requests execution of `action`: if the document contains digital signatures,
    /// sets `pendingWorkbenchAction` and prompts for user confirmation; otherwise proceeds with mutation.
    func requestWorkbenchAction(_ action: WorkbenchAction) {
        guard let session = activeSession, !isWorkbenchProcessing else { return }
        do {
            let preflight = try coreClient.workbenchPreflight(sessionId: session.sessionId)
            workbenchPreflightReport = preflight
            if preflight.isSigned {
                pendingWorkbenchAction = action
                showSignatureConfirmation = true
            } else {
                isWorkbenchProcessing = true
                defer { isWorkbenchProcessing = false }
                let message = try executeWorkbenchAction(action, allowInvalidation: false)
                workbenchNotificationMessage = message
                refreshSessionStateAfterMutation()
            }
        } catch {
            errorMessage = "Preflight audit failed: \(error.localizedDescription)"
        }
    }

    /// User confirmed invalidating signatures: executes the pending action with signature stripping.
    func confirmAndExecutePendingAction() {
        guard let action = pendingWorkbenchAction, !isWorkbenchProcessing else { return }
        pendingWorkbenchAction = nil
        isWorkbenchProcessing = true
        defer { isWorkbenchProcessing = false }
        do {
            let message = try executeWorkbenchAction(action, allowInvalidation: true)
            workbenchNotificationMessage = message
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
        let targets = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.duplicate(targets))
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

    /// Rotates selected pages by `deltaDegrees` (e.g. 90, 180, 270) with signature preflight guards (`M3.1`, `M3.3`).
    func workbenchRotateSelectedPages(deltaDegrees: Int32) {
        guard activeSession != nil, !selectedWorkbenchPages.isEmpty else { return }
        let pagesToRotate = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.rotate(pagesToRotate, deltaDegrees))
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

    /// Prompts the user to pick a PDF document and inserts all its pages after the current selection (`M3.2`).
    func workbenchInsertPages() {
        guard let session = activeSession, !isWorkbenchProcessing else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.pdf]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.message = "Choose a PDF document to insert"
        panel.prompt = "Insert"

        if panel.runModal() == .OK, let url = panel.url {
            let maxSelected = selectedWorkbenchPages.max()
            let insertAt: UInt32 = maxSelected != nil ? (maxSelected! + 1) : session.pdfSummary.pageCount

            isWorkbenchProcessing = true
            defer { isWorkbenchProcessing = false }

            do {
                let data = try Data(contentsOf: url)
                let tempSession = try coreClient.openScopedDocument(at: url)
                let pageCount = tempSession.pdfSummary.pageCount
                _ = try? coreClient.closeSession(sessionId: tempSession.sessionId)
                let sourceIndices = Array(0..<pageCount)

                requestWorkbenchAction(.insert(data, sourceIndices, insertAt))
            } catch {
                errorMessage = "Failed to load PDF for insertion: \(error.localizedDescription)"
            }
        }
    }

    /// Prompts the user to pick one or more PDF documents to merge into the active document (`M3.2`).
    func workbenchMergeDocuments() {
        guard activeSession != nil, !isWorkbenchProcessing else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.pdf]
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.message = "Choose PDF document(s) to merge into the current document"
        panel.prompt = "Merge"

        if panel.runModal() == .OK && !panel.urls.isEmpty {
            let urls = panel.urls
            isWorkbenchProcessing = true
            defer { isWorkbenchProcessing = false }

            do {
                var pdfsData: [Data] = []
                for url in urls {
                    let data = try Data(contentsOf: url)
                    pdfsData.append(data)
                }
                requestWorkbenchAction(.merge(pdfsData))
            } catch {
                errorMessage = "Failed to load PDF(s) for merge: \(error.localizedDescription)"
            }
        }
    }

    /// Prompts the user to choose an export folder and splits the document into standalone PDF parts (`M3.2`).
    func workbenchSplitDocument() {
        guard let session = activeSession, session.pdfSummary.pageCount > 1, !isWorkbenchProcessing else {
            if activeSession?.pdfSummary.pageCount ?? 0 <= 1 {
                errorMessage = "A document must have at least 2 pages to be split."
            }
            return
        }

        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.prompt = "Choose Folder"
        panel.message = "Choose destination directory for split PDF parts"

        if panel.runModal() == .OK, let folderUrl = panel.url {
            let sessionId = session.sessionId
            let pageCount = session.pdfSummary.pageCount
            let selectedPages = selectedWorkbenchPages
            let baseName = session.fileIdentity.displayName.replacingOccurrences(of: ".pdf", with: "")

            let splitRanges: [[UInt32]]
            if !selectedPages.isEmpty && selectedPages.count < Int(pageCount) {
                let selected = Array(selectedPages).sorted()
                let surviving = (0..<pageCount).filter { !selectedPages.contains($0) }
                splitRanges = [selected, surviving]
            } else {
                splitRanges = (0..<pageCount).map { [$0] }
            }

            isWorkbenchProcessing = true
            defer { isWorkbenchProcessing = false }

            do {
                let splitPdfs = try coreClient.splitDocument(
                    sessionId: sessionId,
                    splitRanges: splitRanges
                )

                // Staging in temporary directory to prevent partially completed exports
                let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("vocument_split_\(UUID().uuidString)")
                try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
                defer {
                    try? FileManager.default.removeItem(at: tempDir)
                }

                var stagedFiles: [(URL, URL)] = []
                for (idx, pdfData) in splitPdfs.enumerated() {
                    let fileName = "\(baseName)_part\(idx + 1).pdf"
                    let stagedUrl = tempDir.appendingPathComponent(fileName)
                    try pdfData.write(to: stagedUrl, options: .atomic)

                    // Collision protection: ensure existing files are never silently overwritten
                    var destUrl = folderUrl.appendingPathComponent(fileName)
                    var counter = 1
                    while FileManager.default.fileExists(atPath: destUrl.path) {
                        let candidateName = "\(baseName)_part\(idx + 1)_\(counter).pdf"
                        destUrl = folderUrl.appendingPathComponent(candidateName)
                        counter += 1
                    }
                    stagedFiles.append((stagedUrl, destUrl))
                }

                // Copy all files only after every single part has been successfully staged
                for (stagedUrl, destUrl) in stagedFiles {
                    try FileManager.default.copyItem(at: stagedUrl, to: destUrl)
                }

                workbenchNotificationMessage = "Document split into \(splitPdfs.count) parts in \(folderUrl.lastPathComponent)"
            } catch {
                errorMessage = "Split failed: \(error.localizedDescription)"
            }
        }
    }

    /// Undoes the last committed page mutation via `TransactionManager` (`M3.1`, `UNDO-001`).
    func workbenchUndo() {
        guard let session = activeSession, !isWorkbenchProcessing else { return }
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
        guard let session = activeSession, !isWorkbenchProcessing else { return }
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

    // MARK: - Page Geometry, Crop & Deskew Mutations (`M3.3`, `CROP-001`, `ROTATE-003`)

    /// Applies a non-destructive display crop to selected pages (`CROP-001`).
    func workbenchApplyDisplayCrop(cropRect: FfiPdfRectMilliPt) {
        guard !selectedWorkbenchPages.isEmpty else { return }
        let targets = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.displayCrop(targets, cropRect))
    }

    /// Applies a destructive hard crop with content clipping path injection to selected pages (`CROP-001`).
    func workbenchApplyHardCrop(cropRect: FfiPdfRectMilliPt) {
        guard !selectedWorkbenchPages.isEmpty else { return }
        let targets = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.hardCrop(targets, cropRect))
    }

    /// Automatically detects content margins and applies a crop with `safetyMarginMilliPt` (`CROP-003`).
    func workbenchAutoCropSelectedPages(safetyMarginMilliPt: Int32 = 14170) {
        guard !selectedWorkbenchPages.isEmpty else { return }
        let targets = Array(selectedWorkbenchPages).sorted()
        requestWorkbenchAction(.autoCrop(targets, safetyMarginMilliPt))
    }

    /// Automatically detects deskew angle on the primary selected page and applies fine rotation matrix (`ROTATE-003`).
    func workbenchDeskewSelectedPages() {
        guard let session = activeSession, !selectedWorkbenchPages.isEmpty else { return }
        let primaryPage = selectedWorkbenchPages.sorted().first ?? currentPageIndex
        do {
            let angleDegrees = try coreClient.detectDeskewAngle(
                sessionId: session.sessionId,
                pageIndex: primaryPage
            )
            let angleMilliDegrees = Int32((Double(angleDegrees) * 1000.0).rounded())
            if abs(angleMilliDegrees) < 50 {
                workbenchNotificationMessage = "Document already straight (skew angle < 0.05°)"
                return
            }
            let targets = Array(selectedWorkbenchPages).sorted()
            requestWorkbenchAction(.deskew(targets, angleMilliDegrees))
        } catch {
            errorMessage = "Deskew angle detection failed: \(error.localizedDescription)"
        }
    }
}
